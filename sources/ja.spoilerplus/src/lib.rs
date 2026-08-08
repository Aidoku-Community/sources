#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, ImageResponse, Manga, MangaStatus, Page,
	PageContent, PageContext, Result, Source, Viewer,
	alloc::{string::String, vec::Vec},
	canvas::Rect,
	helpers::uri::{decode_uri, encode_uri_component},
	imports::{
		canvas::{Canvas, ImageRef},
		net::Request,
	},
	prelude::*,
};
use wpcomics::{Cache, Impl, Params, WpComics};

mod helpers;
mod models;

use helpers::*;
use models::*;

const IMG_CDN: &str = "https://img-cdn.stackpathcdn.app";

struct SpoilerPlus;

impl Impl for SpoilerPlus {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			viewer: Viewer::RightToLeft,

			manga_cell: "div.items > div.row > article.item > figure.clearfix",
			manga_cell_image_attr: "abs:data-src",
			manga_parse_id: |url| to_key(url).unwrap_or_default(),

			manga_details_title_transformer: clean_title,
			manga_details_cover_attr: "abs:src",
			manga_details_authors_transformer: |authors| {
				authors.into_iter().filter(|a| a != "更新中").collect()
			},
			// both the genre and the tag row are rendered as li.kind, and each value
			// is its own anchor rather than a delimited string
			manga_details_tags: "ul.list-info > li.kind p.col-xs-8 > a",
			manga_details_tags_splitter: "",
			// the alternative titles row carries the same classes as the publication
			// status row, and only the leading icon tells the two apart
			manga_details_status: "ul.list-info > li.row.status:has(i.fa-rss) > p.col-xs-8",
			status_mapping: |status| match status.trim() {
				"連載中" => MangaStatus::Ongoing,
				"完結" | "完了" => MangaStatus::Completed,
				// an unrecognised value is not a reason to claim the series is running
				_ => MangaStatus::Unknown,
			},

			chapter_parse_id: |url| to_key(&url).unwrap_or_default(),

			datetime_format: "yyyy年MM月dd日",
			datetime_locale: "ja_JP",
			datetime_timezone: "Asia/Tokyo",

			manga_page: |_, manga| url_for(&manga.key),
			page_list_page: |_, _, chapter| url_for(&chapter.key),

			get_search_url: |params, query, page, filters| {
				// searching has its own endpoint that the ordering paths cannot be
				// applied to, so a query takes precedence over the sort filter, which
				// the app hides while searching
				if let Some(query) = query.filter(|query| !query.is_empty()) {
					let query = encode_uri_component(query);
					return Ok(format!("{}?s={query}&page={page}", params.base_url));
				}

				// the site has no sort parameter: each ordering is served from its own
				// path
				Ok(match sort_index(&filters) {
					1 => format!("{}/ranking/{page}/", params.base_url),
					_ => format!("{}/page/{page}/", params.base_url),
				})
			},

			..Default::default()
		}
	}

	fn category_parser(
		&self,
		params: &Params,
		categories: &Option<Vec<String>>,
	) -> (ContentRating, Viewer) {
		let tags = categories.as_deref().unwrap_or(&[]);
		let rating = if tags
			.iter()
			.any(|tag| tag == "オトナ" || tag.contains("エロ"))
		{
			ContentRating::NSFW
		} else if tags.iter().any(|tag| tag == "Ecchi") {
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		};
		(rating, params.viewer)
	}

	fn get_page_list(
		&self,
		cache: &mut Cache,
		params: &Params,
		_manga: Manga,
		chapter: Chapter,
	) -> Result<Vec<Page>> {
		let url = url_for(&chapter.key);
		let html = self.create_request(cache, params, &url, None)?.html()?;

		// the reader holds empty placeholders and asks for the image list over the
		// api, keyed by the ids an inline script declares
		// e.g. <script>window.MangaId =  20466 ;window.CNumber =  417 </script>
		let mut manga_id_opt: Option<String> = None;
		let mut chapter_num_opt: Option<String> = None;
		if let Some(scripts) = html.select("script") {
			for script in scripts {
				// the parser leaves a script node's data empty, so the body has to be
				// read as inner html
				if let Some(data) = script.html() {
					if let Some(id) = read_window_number(&data, "window.MangaId", false) {
						manga_id_opt = Some(id);
					}
					if let Some(number) = read_window_number(&data, "window.CNumber", true) {
						chapter_num_opt = Some(number);
					}
					if manga_id_opt.is_some() && chapter_num_opt.is_some() {
						break;
					}
				}
			}
		}
		let manga_id = manga_id_opt.ok_or_else(|| error!("Manga ID not found in {url}"))?;
		let chapter_num =
			chapter_num_opt.ok_or_else(|| error!("Chapter number not found in {url}"))?;

		let api_url = format!("{}/api/v1/get/c", params.base_url);
		let body = format!("{{\"m\":{manga_id},\"n\":{chapter_num}}}");

		let response = Request::post(&api_url)?
			.body(body)
			.header("Content-Type", "application/json")
			.header("Accept", "application/json, text/plain, */*")
			.header("Referer", &url)
			.send()?
			.get_json::<ChapterApiResponse>()?;

		let ChapterApiResponse {
			c: order_key,
			e: paths,
		} = response;

		if paths.is_empty() {
			bail!("No pages found");
		}

		// every page carries the same descrambling key, so it has to be copied into
		// each page's context
		let pages = paths
			.into_iter()
			.map(|path| {
				let img_url = format!("{IMG_CDN}{path}");
				let mut context = PageContext::new();
				context.insert("key".into(), order_key.clone());
				Page {
					content: PageContent::url_context(img_url, context),
					..Default::default()
				}
			})
			.collect::<Vec<_>>();

		Ok(pages)
	}

	fn process_page_image(
		&self,
		_params: &Params,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		let Some(order_key) = context.get("key").filter(|s| !s.is_empty()) else {
			return Ok(response.image);
		};

		// the site scrambles each page into a square grid and hands the order out as
		// hex bytes xored with its own domain
		const XOR_KEY: &str = "spoilerplus.tv";

		let order_bytes = order_key
			.as_bytes()
			.chunks(2)
			.map(|chunk| {
				core::str::from_utf8(chunk)
					.ok()
					.and_then(|hex| u8::from_str_radix(hex, 16).ok())
					.ok_or_else(|| error!("Invalid order key"))
			})
			.collect::<Result<Vec<u8>>>()?;

		let key_bytes = XOR_KEY.as_bytes();
		let decoded_bytes = order_bytes
			.into_iter()
			.map(|mut byte| {
				for &k in key_bytes {
					byte ^= k;
				}
				byte
			})
			.collect::<Vec<u8>>();

		let parts: Vec<i32> = String::from_utf8(decoded_bytes)
			.map_err(|_| error!("Invalid decoded result"))?
			.split(",")
			.filter_map(|s| s.parse().ok())
			.collect();

		let cols = parts.len().isqrt();
		if cols == 0 || cols * cols != parts.len() {
			return Err(error!("Page order is not a square grid"));
		}

		let image_width = response.image.width();
		let image_height = response.image.height();

		let mut canvas = Canvas::new(image_width, image_height);

		let unit_width = image_width / cols as f32;
		let unit_height = image_height / cols as f32;

		for (i, pos) in parts.iter().enumerate() {
			let sx = (*pos % cols as i32) as f32 * unit_width;
			let sy = (*pos / cols as i32) as f32 * unit_height;

			let dx = (i % cols) as f32 * unit_width;
			let dy = (i / cols) as f32 * unit_height;

			let src_rect = Rect::new(sx, sy, unit_width, unit_height);
			let dst_rect = Rect::new(dx, dy, unit_width, unit_height);

			canvas.copy_image(&response.image, src_rect, dst_rect);
		}

		Ok(canvas.get_image())
	}

	fn handle_deep_link(
		&self,
		_cache: &mut Cache,
		params: &Params,
		url: String,
	) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};
		// keys are stored decoded, and a shared link carries the encoded form
		let key = decode_uri(path);

		// series live at the site root, so the slug suffix is what separates them
		// from the genre, tag and ranking paths
		// Series:  /TITLE-raw-free/
		// Chapter: /TITLE-raw-free/第N話/
		const SERIES_SUFFIX: &str = "-raw-free";

		let trimmed = key.trim_end_matches('/');
		let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();

		let Some(series) = segments.first().filter(|s| s.ends_with(SERIES_SUFFIX)) else {
			return Ok(None);
		};
		let manga_key = format!("/{series}/");

		if segments.len() > 1 {
			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: format!("{trimmed}/"),
			}))
		} else {
			Ok(Some(DeepLinkResult::Manga { key: manga_key }))
		}
	}
}

fn sort_index(filters: &[FilterValue]) -> i32 {
	filters
		.iter()
		.find_map(|filter| match filter {
			FilterValue::Sort { index, .. } => Some(*index),
			_ => None,
		})
		.unwrap_or(0)
}

fn read_window_number(data: &str, name: &str, fractional: bool) -> Option<String> {
	let after = &data[data.find(name)? + name.len()..];
	let after_eq = after[after.find('=')? + 1..].trim_start();
	let end = after_eq
		.find(|c: char| !c.is_ascii_digit() && !(fractional && c == '.'))
		.unwrap_or(after_eq.len());
	let number = after_eq[..end].trim();
	(!number.is_empty()).then(|| number.into())
}

register_source!(
	WpComics<SpoilerPlus>,
	PageImageProcessor,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test {
	use super::*;
	use aidoku::{DeepLinkHandler, MangaPageResult, alloc::vec};
	use aidoku_test::aidoku_test;

	const SORT_UPDATED: i32 = 0;
	const SORT_RANKING: i32 = 1;

	const SERIES_KEY: &str = "/HUNTER X HUNTER-raw-free/";

	fn source() -> WpComics<SpoilerPlus> {
		WpComics::new()
	}

	fn sort(index: i32) -> Vec<FilterValue> {
		vec![FilterValue::Sort {
			id: String::from("sort"),
			index,
			ascending: false,
		}]
	}

	fn browse(index: i32, page: i32) -> MangaPageResult {
		source()
			.get_search_manga_list(None, page, sort(index))
			.expect("browse request should succeed")
	}

	fn series() -> Manga {
		source()
			.get_manga_update(
				Manga {
					key: SERIES_KEY.into(),
					..Default::default()
				},
				true,
				true,
			)
			.expect("details request should succeed")
	}

	fn leading_keys(result: &MangaPageResult) -> Vec<&String> {
		result
			.entries
			.iter()
			.take(5)
			.map(|manga| &manga.key)
			.collect()
	}

	// The app sends no filter value until one is picked, so the fallback has to be
	// a real ordering rather than an out of range option.
	#[aidoku_test]
	fn test_sort_index_falls_back_to_the_first_option() {
		assert_eq!(sort_index(&[]), SORT_UPDATED);
		assert_eq!(sort_index(&sort(SORT_RANKING)), SORT_RANKING);
	}

	#[aidoku_test]
	fn test_sort_updated() {
		assert!(
			!browse(SORT_UPDATED, 1).entries.is_empty(),
			"updates ordering should return entries"
		);
	}

	#[aidoku_test]
	fn test_sort_updated_page_2() {
		assert!(
			!browse(SORT_UPDATED, 2).entries.is_empty(),
			"updates ordering page 2 should return entries"
		);
	}

	#[aidoku_test]
	fn test_sort_ranking() {
		assert!(
			!browse(SORT_RANKING, 1).entries.is_empty(),
			"ranking ordering should return entries"
		);
	}

	#[aidoku_test]
	fn test_browse_without_filters() {
		let result = source()
			.get_search_manga_list(None, 1, Vec::new())
			.expect("browse request should succeed");
		assert!(!result.entries.is_empty(), "browse should return entries");
	}

	#[aidoku_test]
	fn test_search() {
		let result = source()
			.get_search_manga_list(Some(String::from("ワンピース")), 1, Vec::new())
			.expect("search request should succeed");
		assert!(!result.entries.is_empty(), "search should return entries");
	}

	#[aidoku_test]
	fn test_query_takes_precedence_over_sort() {
		let searched = source()
			.get_search_manga_list(Some(String::from("ワンピース")), 1, sort(SORT_RANKING))
			.expect("search request should succeed");
		let ranking = browse(SORT_RANKING, 1);
		assert!(!searched.entries.is_empty(), "search should return entries");
		assert_ne!(
			leading_keys(&searched),
			leading_keys(&ranking),
			"a query should search rather than fall back to the ranking path"
		);
	}

	#[aidoku_test]
	fn test_empty_query_falls_through_to_the_sort() {
		let result = source()
			.get_search_manga_list(Some(String::new()), 1, sort(SORT_RANKING))
			.expect("browse request should succeed");
		let ranking = browse(SORT_RANKING, 1);
		assert!(!result.entries.is_empty(), "browse should return entries");
		assert_eq!(
			leading_keys(&result),
			leading_keys(&ranking),
			"an empty query should use the sort path"
		);
	}

	// The updates ordering starts at the site home page, which stacks a carousel
	// and a ranking block around the paginated list.
	#[aidoku_test]
	fn test_updated_page_1_holds_only_the_paginated_block() {
		let result = browse(SORT_UPDATED, 1);

		let mut keys = result
			.entries
			.iter()
			.map(|manga| manga.key.as_str())
			.collect::<Vec<_>>();
		let total = keys.len();
		keys.sort_unstable();
		keys.dedup();
		assert_eq!(keys.len(), total, "a page should not repeat entries");

		// one block currently holds 24 entries
		assert!(
			total <= 40,
			"a page should hold a single block, got {total} entries"
		);
	}

	#[aidoku_test]
	fn test_keys_stay_relative_and_covers_absolute() {
		let result = browse(SORT_UPDATED, 1);

		for manga in &result.entries {
			assert!(
				manga.key.starts_with('/') && manga.key.ends_with("-raw-free/"),
				"key should be a site-relative series path, got {}",
				manga.key
			);
			let cover = manga.cover.as_ref().expect("entry should have a cover");
			assert!(
				cover.starts_with(BASE_URL),
				"cover should be an absolute url, got {cover}"
			);
		}
	}

	#[aidoku_test]
	fn test_pagination_ends() {
		let result = browse(SORT_UPDATED, 9999);
		assert!(
			result.entries.is_empty() && !result.has_next_page,
			"out of range page should end pagination"
		);
	}

	#[aidoku_test]
	fn test_sorts_return_different_orders() {
		let updated = browse(SORT_UPDATED, 1);
		let ranking = browse(SORT_RANKING, 1);
		assert_ne!(
			leading_keys(&updated),
			leading_keys(&ranking),
			"the sort options should not resolve to the same order"
		);
	}

	#[aidoku_test]
	fn test_manga_details() {
		let manga = series();
		assert_eq!(
			manga.title, "HUNTER X HUNTER",
			"title should drop the suffix"
		);
		let cover = manga.cover.expect("series should have a cover");
		assert!(
			cover.starts_with(BASE_URL),
			"cover should be an absolute url, got {cover}"
		);
		assert!(
			manga.description.is_some_and(|d| !d.is_empty()),
			"series should have a description"
		);
		assert!(
			manga.tags.is_some_and(|t| !t.is_empty()),
			"series should have tags"
		);
	}

	#[aidoku_test]
	fn test_status_is_not_read_from_the_alternative_titles_row() {
		assert_eq!(
			series().status,
			MangaStatus::Ongoing,
			"a running series should not be read off the alternative titles row"
		);
	}

	#[aidoku_test]
	fn test_chapter_list() {
		let chapters = series().chapters.expect("series should have chapters");
		assert!(chapters.len() >= 400, "got {} chapters", chapters.len());

		let first = chapters.first().expect("chapter list should not be empty");
		assert!(
			first.key.starts_with(SERIES_KEY),
			"chapter key should sit under the series path, got {}",
			first.key
		);
		assert!(
			first.chapter_number.is_some(),
			"chapter number should be parsed out of the title"
		);
	}

	#[aidoku_test]
	fn test_page_list() {
		let chapters = series().chapters.expect("series should have chapters");
		let chapter = chapters
			.into_iter()
			.next()
			.expect("chapter list should not be empty");
		let pages = source()
			.get_page_list(Manga::default(), chapter)
			.expect("page list request should succeed");

		assert!(!pages.is_empty(), "chapter should have pages");
		for page in &pages {
			let PageContent::Url(url, context) = &page.content else {
				panic!("page should be a url");
			};
			assert!(
				url.starts_with(IMG_CDN),
				"page should be served from the image cdn, got {url}"
			);
			// the descrambling order travels with the page, so a missing key would
			// silently ship scrambled images
			assert!(
				context
					.as_ref()
					.and_then(|c| c.get("key"))
					.is_some_and(|key| !key.is_empty()),
				"page should carry the descrambling key"
			);
		}
	}

	#[aidoku_test]
	fn test_deep_links() {
		let handle = |url: &str| {
			source()
				.handle_deep_link(String::from(url))
				.expect("deep link should be handled")
		};

		assert_eq!(
			handle("https://spoilerplus.tv/HUNTER%20X%20HUNTER-raw-free/"),
			Some(DeepLinkResult::Manga {
				key: SERIES_KEY.into()
			})
		);
		assert_eq!(
			handle("https://spoilerplus.tv/HUNTER%20X%20HUNTER-raw-free/第417話/"),
			Some(DeepLinkResult::Chapter {
				manga_key: SERIES_KEY.into(),
				key: "/HUNTER X HUNTER-raw-free/第417話/".into(),
			})
		);
		assert_eq!(handle("https://spoilerplus.tv/genre/Ecchi/"), None);
		assert_eq!(handle("https://spoilerplus.tv/ranking/"), None);
		assert_eq!(handle("https://example.com/HUNTER-raw-free/"), None);
	}
}
