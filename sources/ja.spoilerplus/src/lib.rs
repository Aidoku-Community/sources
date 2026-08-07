#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	ImageResponse, Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext,
	PageImageProcessor, Result, Source, Viewer,
	alloc::{string::String, vec::Vec},
	canvas::Rect,
	helpers::uri::{decode_uri, encode_uri_component},
	imports::{
		canvas::{Canvas, ImageRef},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};

mod helpers;
mod models;

use helpers::*;
use models::*;

const IMG_CDN: &str = "https://img-cdn.stackpathcdn.app";

struct SpoilerPlus;

impl Source for SpoilerPlus {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// searching has its own endpoint that the ordering paths cannot be applied
		// to, so a query takes precedence over the sort filter, which the app hides
		// while searching
		if let Some(query) = query.filter(|query| !query.is_empty()) {
			let query = encode_uri_component(query);
			return parse_listing_page(&format!("{BASE_URL}?s={query}&page={page}"));
		}

		// the site has no sort parameter: each ordering is served from its own path
		let url = match sort_index(&filters) {
			// all-time view count, descending
			1 => format!("{BASE_URL}/ranking/{page}/"),
			// most recently updated chapters first
			_ => format!("{BASE_URL}/page/{page}/"),
		};
		parse_listing_page(&url)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = url_for(&manga.key);
		let html = Request::get(&manga_url)?.html()?;

		if needs_details {
			manga.title = clean_title(
				html.select_first("h1.title-detail")
					.and_then(|e| e.text())
					.unwrap_or(manga.title),
			);
			manga.cover = html
				.select_first(".detail-info .col-image img")
				.and_then(|el| el.attr("src"))
				.map(|src| absolute_url(&src));
			manga.authors = html
				.select_first("ul.list-info > li.author > p.col-xs-8")
				.and_then(|el| el.text())
				.filter(|author| !author.is_empty() && author != "更新中")
				.map(|author| Vec::from([author]));
			// the summary sits in a nested <p>, which the parser flattens into
			// siblings, so the blocks are joined back together here
			manga.description = html.select(".detail-content p").and_then(|els| {
				let texts: Vec<String> = els
					.filter_map(|el| el.own_text())
					.filter(|text| !text.is_empty())
					.collect();
				if texts.is_empty() {
					None
				} else {
					Some(texts.join("\n"))
				}
			});
			manga.url = Some(manga_url);
			manga.tags = html
				.select("ul.list-info > li.kind p.col-xs-8 > a")
				.map(|els| {
					let mut tags = els.filter_map(|el| el.text()).collect::<Vec<String>>();
					tags.sort();
					tags.dedup();
					tags
				});
			manga.status = parse_status(&html);
			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags.iter().any(|e| e == "オトナ" || e.contains("エロ"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = Viewer::RightToLeft;

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = html
				.select("div.list-chapter > nav > ul > li div.chapter > a")
				.map(|elements| {
					elements
						.filter_map(|element| {
							let key = to_key(&element.attr("href")?)?;
							let title_text = element.text()?;
							let chapter_number = extract_ch_number(&title_text);
							Some(Chapter {
								url: Some(url_for(&key)),
								key,
								chapter_number,
								..Default::default()
							})
						})
						.collect::<Vec<_>>()
				});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = url_for(&chapter.key);
		let html = Request::get(&url)?.html()?;

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

		// Fetch image URL list via JSON API
		let api_url = format!("{BASE_URL}/api/v1/get/c");
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

		// every page carries the same descrambling key, so it has to be copied
		// into each page's context
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
}

/// The selected option of the sort filter, falling back to the first one.
fn sort_index(filters: &[FilterValue]) -> i32 {
	filters
		.iter()
		.find_map(|filter| match filter {
			FilterValue::Sort { index, .. } => Some(*index),
			_ => None,
		})
		.unwrap_or(0)
}

/// Read a numeric `window.<name> = <number>` assignment out of a script body.
fn read_window_number(data: &str, name: &str, fractional: bool) -> Option<String> {
	let after = &data[data.find(name)? + name.len()..];
	let after_eq = after[after.find('=')? + 1..].trim_start();
	let end = after_eq
		.find(|c: char| !c.is_ascii_digit() && !(fractional && c == '.'))
		.unwrap_or(after_eq.len());
	let number = after_eq[..end].trim();
	(!number.is_empty()).then(|| number.into())
}

/// The publication status, read off the info row labelled 状態.
///
/// The label is what identifies the row: the alternative titles row carries the
/// same "row status" classes, so keying on the class alone reads that instead.
fn parse_status(html: &aidoku::imports::html::Document) -> MangaStatus {
	let Some(rows) = html.select("ul.list-info > li.status") else {
		return MangaStatus::Unknown;
	};
	for row in rows {
		let is_status_row = row
			.select_first("p.name")
			.and_then(|el| el.text())
			.is_some_and(|label| label.contains("状態"));
		if !is_status_row {
			continue;
		}
		let Some(value) = row.select_first("p.col-xs-8").and_then(|el| el.text()) else {
			continue;
		};
		return match value.trim() {
			"連載中" => MangaStatus::Ongoing,
			"完結" | "完了" => MangaStatus::Completed,
			// an unrecognised value is not a reason to claim the series is running
			_ => MangaStatus::Unknown,
		};
	}
	MangaStatus::Unknown
}

/// Scrapes a paginated listing page into manga entries.
///
/// Every listing renders the paginated block as ".items", while the home page
/// stacks a carousel and a ranking block around it. Entries are scoped to the
/// block so those extras do not get mixed into the ordering.
fn parse_listing_page(url: &str) -> Result<MangaPageResult> {
	let html = Request::get(url)?.html()?;

	// an exhausted listing still renders an empty block, so a missing block means
	// the page did not load rather than that there is nothing left to show
	let list = html
		.select_first("div.items")
		.ok_or_else(|| error!("Manga list not found"))?;

	let entries = list
		.select("article.item")
		.map(|elements| {
			elements
				.filter_map(|element| {
					let link = element.select_first("figcaption h3 > a")?;
					let key = to_key(&link.attr("href")?)?;
					let title = link.text()?;
					// the plain src is a placeholder until the lazy loader runs
					let cover = element
						.select_first("div.image img")
						.and_then(|img| img.attr("data-src"))
						.map(|src| absolute_url(&src));
					Some(Manga {
						url: Some(url_for(&key)),
						key,
						title: clean_title(title),
						cover,
						..Default::default()
					})
				})
				.collect::<Vec<Manga>>()
		})
		.unwrap_or_default();

	let has_next_page = !entries.is_empty();

	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

impl PageImageProcessor for SpoilerPlus {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		let Some(order_key) = context.get("key").filter(|s| !s.is_empty()) else {
			return Ok(response.image);
		};

		// the site scrambles each page into a square grid and hands the order out
		// as hex bytes xored with its own domain
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
}

impl ImageRequestProvider for SpoilerPlus {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for SpoilerPlus {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
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

register_source!(
	SpoilerPlus,
	PageImageProcessor,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test {
	use super::*;
	use aidoku::alloc::vec;
	use aidoku_test::aidoku_test;

	const SORT_UPDATED: i32 = 0;
	const SORT_RANKING: i32 = 1;

	/// A long running series, used wherever a test needs a stable entry.
	const SERIES_KEY: &str = "/HUNTER X HUNTER-raw-free/";

	fn sort(index: i32) -> Vec<FilterValue> {
		vec![FilterValue::Sort {
			id: String::from("sort"),
			index,
			ascending: false,
		}]
	}

	fn browse(index: i32, page: i32) -> MangaPageResult {
		SpoilerPlus
			.get_search_manga_list(None, page, sort(index))
			.expect("browse request should succeed")
	}

	fn series() -> Manga {
		SpoilerPlus
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

	/// The leading keys of a result, which is what an ordering actually changes.
	fn leading_keys(result: &MangaPageResult) -> Vec<&String> {
		result
			.entries
			.iter()
			.take(5)
			.map(|manga| &manga.key)
			.collect()
	}

	/// The app sends no filter value until one is picked, so the fallback has to
	/// be a real ordering rather than an out of range option.
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
		let result = SpoilerPlus
			.get_search_manga_list(None, 1, Vec::new())
			.expect("browse request should succeed");
		assert!(!result.entries.is_empty(), "browse should return entries");
	}

	#[aidoku_test]
	fn test_search() {
		let result = SpoilerPlus
			.get_search_manga_list(Some(String::from("ワンピース")), 1, Vec::new())
			.expect("search request should succeed");
		assert!(!result.entries.is_empty(), "search should return entries");
	}

	/// The search endpoint takes no ordering, so a query has to win over whatever
	/// sort value is still stored while the filter is hidden.
	#[aidoku_test]
	fn test_query_takes_precedence_over_sort() {
		let searched = SpoilerPlus
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

	/// An empty query is not a search, so it has to fall through to the ordering
	/// paths instead of hitting the search endpoint with nothing.
	#[aidoku_test]
	fn test_empty_query_falls_through_to_the_sort() {
		let result = SpoilerPlus
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

	/// The updates ordering starts at the site home page, which stacks a carousel
	/// and a ranking block around the paginated list. Scraping outside ".items"
	/// mixes those into the page and repeats entries, so guard against that.
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

	/// Keys are site-relative paths, which is what the listing hrefs already hold,
	/// while covers have to be joined into absolute urls to be fetchable.
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

	/// Pages past the end return no entries, which is how the app learns to stop
	/// paginating.
	#[aidoku_test]
	fn test_pagination_ends() {
		let result = browse(SORT_UPDATED, 9999);
		assert!(
			result.entries.is_empty() && !result.has_next_page,
			"out of range page should end pagination"
		);
	}

	/// Each option has to actually change the order, otherwise the filter is a
	/// no-op and everything falls back to one ordering.
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

	/// The info list labels the alternative titles row with the same "status"
	/// class as the publication status, so keying on the class alone reads the
	/// title as the status and drops it to unknown.
	#[aidoku_test]
	fn test_status_is_read_from_the_labelled_row() {
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
		let pages = SpoilerPlus
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

	/// Series live at the site root, so the deep link handler has to tell them
	/// apart from the genre, tag and ranking paths by their slug suffix.
	#[aidoku_test]
	fn test_deep_links() {
		let handle = |url: &str| {
			SpoilerPlus
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
