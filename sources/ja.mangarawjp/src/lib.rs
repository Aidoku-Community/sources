#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	ImageResponse, Manga, MangaPageResult, Page, PageContent, PageContext, PageImageProcessor,
	Result, Source, Viewer,
	alloc::{string::String, vec::Vec},
	canvas::Rect,
	helpers::uri::encode_uri_component,
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

const BASE_URL: &str = "https://mangarawjp.tv";
const IMG_CDN: &str = "https://img-cdn.stackpathcdn.app";

struct MangaRawJP;

impl Source for MangaRawJP {
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
			// most recently updated chapters first; page 1 is the site home page
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
		let manga_url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&manga_url)?.html()?;

		if needs_details {
			manga.title = clean_title(
				html.select_first("h1")
					.and_then(|e| e.text())
					.unwrap_or(manga.title),
			);
			manga.cover = html
				.select_first(".post-cover > img")
				.and_then(|el| el.attr("abs:src"));
			manga.description = html.select(".page-h p").and_then(|els| {
				let texts: Vec<String> = els.filter_map(|el| el.own_text()).collect();
				if texts.is_empty() {
					None
				} else {
					Some(texts.join("\n "))
				}
			});
			manga.url = Some(manga_url);
			manga.tags = html.select(".category-warp > a, .tag-list > a").map(|els| {
				let mut tags = els.filter_map(|el| el.text()).collect::<Vec<String>>();
				tags.sort();
				tags.dedup();
				tags
			});
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
			manga.chapters = html.select(".ch-list li a").map(|elements| {
				elements
					.filter_map(|element| {
						let url = element.attr("abs:href")?;
						let key = url.strip_prefix(BASE_URL)?.into();
						let title_text = element.text()?;
						let chapter_number = extract_ch_number(&title_text);
						Some(Chapter {
							key,
							chapter_number,
							url: Some(url),
							..Default::default()
						})
					})
					.collect::<Vec<_>>()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(&url)?.html()?;

		// Extract window.MangaId and window.CNumber from inline script tags
		// e.g. <script>window.MangaId =  133 ;window.CNumber =  10 </script>
		let mut manga_id_opt: Option<String> = None;
		let mut chapter_num_opt: Option<String> = None;
		if let Some(scripts) = html.select("script") {
			for script in scripts {
				if let Some(data) = script.data() {
					// Look for window.MangaId
					if let Some(pos) = data.find("window.MangaId") {
						let after = &data[pos + 14..]; // after "window.MangaId"
						if let Some(eq_pos) = after.find('=') {
							let after_eq = after[eq_pos + 1..].trim_start();
							let end = after_eq
								.find(|c: char| !c.is_ascii_digit())
								.unwrap_or(after_eq.len());
							let num_str = after_eq[..end].trim();
							if !num_str.is_empty() {
								manga_id_opt = Some(num_str.into());
							}
						}
					}
					// Look for window.CNumber
					if let Some(pos) = data.find("window.CNumber") {
						let after = &data[pos + 14..]; // after "window.CNumber"
						if let Some(eq_pos) = after.find('=') {
							let after_eq = after[eq_pos + 1..].trim_start();
							let end = after_eq
								.find(|c: char| !c.is_ascii_digit() && c != '.')
								.unwrap_or(after_eq.len());
							let num_str = after_eq[..end].trim();
							if !num_str.is_empty() {
								chapter_num_opt = Some(num_str.into());
							}
						}
					}
					if manga_id_opt.is_some() && chapter_num_opt.is_some() {
						break;
					}
				}
			}
		}
		let manga_id = manga_id_opt.ok_or_else(|| error!("Manga ID not found"))?;
		let chapter_num = chapter_num_opt.ok_or_else(|| error!("Chapter number not found"))?;

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

		let pages = response
			.e
			.into_iter()
			.map(|path| {
				let img_url = format!("{IMG_CDN}{path}");
				let mut context = PageContext::new();
				context.insert("key".into(), response.c.clone());
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

/// Scrapes a paginated listing page into manga entries.
///
/// The first page of the updates ordering is the site home page, which stacks
/// several ".post-list" blocks: the updates list, then the ranking and one block
/// per featured genre. Only the first block is the paginated list, so entries
/// are always scoped to it — scraping every block there mixes the rankings and
/// genre picks into the updates order.
fn parse_listing_page(url: &str) -> Result<MangaPageResult> {
	let html = Request::get(url)?.html()?;

	// an exhausted listing still renders an empty block, so a missing block means
	// the page did not load rather than that there is nothing left to show
	let list = html
		.select_first(".post-list")
		.ok_or_else(|| error!("Manga list not found"))?;

	let entries = list
		.select("a")
		.map(|elements| {
			elements
				.filter_map(|element| {
					let url = element.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL).map(String::from)?;
					let title = element.select_first("h3")?.text()?;
					// the plain src is a base64 placeholder until the lazy loader runs
					let cover = element
						.select_first("img")
						.and_then(|img| img.attr("abs:data-src"));
					Some(Manga {
						key,
						title: clean_title(title),
						cover,
						url: Some(url),
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

impl PageImageProcessor for MangaRawJP {
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

		const XOR_KEY: &str = "mangarawjp.tv";

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
				Ok(byte)
			})
			.collect::<Result<Vec<u8>>>()?;

		let parts: Vec<i32> = String::from_utf8(decoded_bytes)
			.map_err(|_| error!("Invalid decoded result"))?
			.split(",")
			.filter_map(|s| s.parse().ok())
			.collect();

		let cols = parts.len().isqrt();

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

impl ImageRequestProvider for MangaRawJP {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for MangaRawJP {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(key) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const SERIES_PATH: &str = "/manga-raw/";

		if key.starts_with(SERIES_PATH) {
			// Determine chapter vs series URL by path segment count
			// Series:  /manga-raw/TITLE-raw-free/
			// Chapter: /manga-raw/TITLE-raw-free/第N話/
			let trimmed = key.trim_end_matches('/');
			let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();

			if segments.len() > 2 {
				// Chapter URL — derive manga key from parent path
				let manga_key = segments[..2].join("/");
				let manga_key = format!("/{manga_key}/");
				Ok(Some(DeepLinkResult::Chapter {
					manga_key,
					key: key.into(),
				}))
			} else {
				// Series URL
				Ok(Some(DeepLinkResult::Manga { key: key.into() }))
			}
		} else {
			Ok(None)
		}
	}
}

register_source!(
	MangaRawJP,
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

	fn sort(index: i32) -> Vec<FilterValue> {
		vec![FilterValue::Sort {
			id: String::from("sort"),
			index,
			ascending: false,
		}]
	}

	fn browse(index: i32, page: i32) -> MangaPageResult {
		MangaRawJP
			.get_search_manga_list(None, page, sort(index))
			.expect("browse request should succeed")
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
		let result = MangaRawJP
			.get_search_manga_list(None, 1, Vec::new())
			.expect("browse request should succeed");
		assert!(!result.entries.is_empty(), "browse should return entries");
	}

	#[aidoku_test]
	fn test_search() {
		let result = MangaRawJP
			.get_search_manga_list(Some(String::from("ワンピース")), 1, Vec::new())
			.expect("search request should succeed");
		assert!(!result.entries.is_empty(), "search should return entries");
	}

	/// The search endpoint takes no ordering, so a query has to win over whatever
	/// sort value is still stored while the filter is hidden. Matching the query
	/// against the titles would not catch a fallback, since the query is also the
	/// top ranking entry, so the result is compared against the ranking page.
	#[aidoku_test]
	fn test_query_takes_precedence_over_sort() {
		let searched = MangaRawJP
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
		let result = MangaRawJP
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

	/// The updates ordering starts at the site home page, which stacks the updates
	/// block together with the ranking and featured genre blocks. Scraping every
	/// ".post-list" there mixes all four into one page and repeats entries, so
	/// guard against that regression.
	#[aidoku_test]
	fn test_updated_page_1_holds_only_the_updates_block() {
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

		// one block currently holds 24 entries; the mixed-in version yields 84
		assert!(
			total <= 40,
			"a page should hold a single block, got {total} entries"
		);
	}

	/// Keys are site-relative paths, which is what "/manga-raw/..." hrefs already
	/// hold, while covers have to be joined into absolute urls to be fetchable.
	#[aidoku_test]
	fn test_keys_stay_relative_and_covers_absolute() {
		let result = browse(SORT_UPDATED, 1);

		for manga in &result.entries {
			assert!(
				manga.key.starts_with("/manga-raw/"),
				"key should be a site-relative path, got {}",
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
}
