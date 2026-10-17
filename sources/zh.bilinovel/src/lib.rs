#![no_std]

mod home;
mod html;
mod markdown;
mod net;
mod restore;
mod tags;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::net::Request,
	imports::std::send_partial_result,
	prelude::*,
};

use html::{ChapterPage as _, MangaPage as _};
use net::Url;

pub const BASE_URL: &str = "https://www.bilinovel.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
	 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
/// Chapters are split into paginated fragments; cap the page count so a
/// malformed chapter cannot fetch forever.
const MAX_CHAPTER_PAGES: i32 = 50;
/// Number of fragments speculatively fetched per parallel batch. A batch
/// covers typical chapters in a single round trip; longer chapters extend
/// with one more batch per `BATCH_SIZE` fragments.
const BATCH_SIZE: i32 = 8;

struct Bilinovel;

impl Source for Bilinovel {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = Url::from_query_or_filters(query.as_deref(), page, filters)?;
		let document = url.request()?.html()?;
		document.manga_page_result()
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let document = Url::manga(&manga.key).request()?.html()?;
			document.update_details(&mut manga)?;
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let document = Url::catalog(&manga.key).request()?.html()?;
			manga.chapters = Some(document.chapters()?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// The catalogue lists some chapters as placeholder keys
		// (`v{volume}-{index}`); the volume page has their real links.
		let chapter_key = match html::parse_placeholder_key(&chapter.key) {
			Some((volume, index)) => {
				let document = Url::volume(&manga.key, volume).request()?.html()?;
				html::volume_chapter(&document, index).ok_or_else(|| {
					error!("no chapter {index} on volume {volume} of {}", manga.key)
				})?
			}
			None => chapter.key,
		};

		let mut fragments: Vec<(i32, String)> = Vec::new();
		let mut seen_images: Vec<String> = Vec::new();

		// The first page carries the shuffle inputs: the chapter id seeds
		// the paragraph permutation, and the (versioned) chapterlog script
		// supplies its parameters.
		let page_html = Url::chapter(&manga.key, &chapter_key)
			.request()?
			.send()?
			.get_string()?;
		if html::is_withheld(&page_html) {
			bail!(
				"chapter {} of {}: the site withheld paragraphs",
				chapter_key,
				manga.key
			);
		}
		let chapter_id = restore::parse_chapter_id(&page_html);
		let params = html::chapterlog_src(&page_html).and_then(restore::shuffle_params);
		html::extract_fragment(
			&page_html,
			1,
			chapter_id,
			params.as_ref(),
			&mut fragments,
			&mut seen_images,
		);

		// Probe page 2: every fragment title from the second page on
		// carries (page/total), so one request reveals the exact fragment
		// count and the rest can be fetched in a single precise parallel
		// batch, with no wasted out-of-range stub requests.
		let mut total: Option<i32> = None;
		let probe_failed = match Url::chapter_page(&manga.key, &chapter_key, 2)
			.request()?
			.send()
		{
			Ok(response) => {
				let html = response.get_string().unwrap_or_default();
				match html::fragment_position(&html) {
					Some((page, page_total)) if page <= page_total => {
						total = Some(page_total);
						html::extract_fragment(
							&html,
							page,
							chapter_id,
							params.as_ref(),
							&mut fragments,
							&mut seen_images,
						);
						false
					}
					// Out-of-range stub like (2/1): single-page chapter.
					_ => false,
				}
			}
			Err(_) => true,
		};

		if probe_failed {
			// Fall back to speculative parallel batches.
			total = fetch_batches(
				&manga.key,
				&chapter_key,
				chapter_id,
				params.as_ref(),
				&mut fragments,
				&mut seen_images,
			);
		}

		if let Some(total) = total {
			let end = total.min(MAX_CHAPTER_PAGES);
			if end > 2 {
				let requests: Vec<Request> = (3..=end)
					.filter_map(|page| {
						Url::chapter_page(&manga.key, &chapter_key, page)
							.request()
							.ok()
					})
					.collect();
				for result in Request::send_all(requests) {
					let Ok(response) = result else {
						continue;
					};
					let html = response.get_string().unwrap_or_default();
					let Some((page, _)) = html::fragment_position(&html) else {
						continue;
					};
					html::extract_fragment(
						&html,
						page,
						chapter_id,
						params.as_ref(),
						&mut fragments,
						&mut seen_images,
					);
				}
			}

			// Backfill fragments lost to a transient failure; a page that
			// keeps failing fails the chapter rather than delivering text
			// with a gap.
			for page in 2..=end {
				if fragments.iter().any(|(number, _)| *number == page) {
					continue;
				}
				let response = Url::chapter_page(&manga.key, &chapter_key, page)
					.request()?
					.send()?;
				let html = response.get_string().unwrap_or_default();
				html::extract_fragment(
					&html,
					page,
					chapter_id,
					params.as_ref(),
					&mut fragments,
					&mut seen_images,
				);
			}
		}

		// One text page with the illustrations inline in their original
		// positions (device-verified: mixed text/image page lists drop the
		// text, `data:` images do not render, and Markdown images cannot
		// carry the Referer the image host wants — hence the site's own
		// attachment copies, see `attachment_url`).
		// NOTE: Aidoku's paginated text reader parses Markdown through
		// Foundation AttributedString, which drops images; the inline
		// illustrations only render in the scroll text reader (MarkdownUI).
		fragments.sort_by_key(|(page, _)| *page);
		if fragments.is_empty() {
			bail!("chapter {chapter_key} of {} is empty", manga.key);
		}
		let text = fragments
			.into_iter()
			.map(|(_, text)| text)
			.collect::<Vec<_>>()
			.join("\n\n");
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl ListingProvider for Bilinovel {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"latest" => Url::wenku("lastupdate", page),
			"new" => Url::wenku("postdate", page),
			"popular" => Url::top("monthvisit", page),
			"completed" => Url::topfull("weekvote", page),
			_ => bail!("Unknown listing: {}", listing.id),
		};
		let document = url.request()?.html()?;
		document.manga_page_result()
	}
}

impl ImageRequestProvider for Bilinovel {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Bilinovel {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split(['?', '#'])
			.next()
			.unwrap_or(&url)
			.rsplit("bilinovel.com")
			.next()
			.unwrap_or("")
			.trim_start_matches('/');

		let mut parts = path.splitn(3, '/');
		if parts.next() != Some("novel") {
			return Ok(None);
		}

		let Some(second) = parts.next() else {
			return Ok(None);
		};
		let manga_key = second.trim_end_matches(".html");
		if manga_key.is_empty() || !manga_key.chars().all(|c| c.is_ascii_digit()) {
			return Ok(None);
		}

		match parts.next() {
			// /novel/{id}.html, /novel/{id} or /novel/{id}/catalog
			None | Some("catalog") => Ok(Some(DeepLinkResult::Manga {
				key: manga_key.into(),
			})),
			// /novel/{id}/{chapter}_{n}.html or /novel/{id}/{chapter}.html
			Some(chapter_part) => {
				let key = chapter_part
					.trim_end_matches(".html")
					.split('_')
					.next()
					.unwrap_or("")
					.to_string();
				if key.is_empty() || !key.chars().all(|c| c.is_ascii_digit()) {
					return Ok(None);
				}
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key,
				}))
			}
		}
	}
}

/// Speculatively fetch fragments in parallel batches when the page-2
/// probe fails, returning the highest page total observed. Out-of-range
/// guesses render as stubs like `（8/7）` and are discarded; the loop
/// only continues while the highest requested fragment exists.
fn fetch_batches(
	manga_key: &str,
	chapter_key: &str,
	chapter_id: Option<i64>,
	params: Option<&restore::ShuffleParams>,
	fragments: &mut Vec<(i32, String)>,
	seen_images: &mut Vec<String>,
) -> Option<i32> {
	let mut start = 2;
	let mut total: Option<i32> = None;
	while start <= MAX_CHAPTER_PAGES {
		let end = (start + BATCH_SIZE - 1).min(MAX_CHAPTER_PAGES);
		let requests: Vec<Request> = (start..=end)
			.filter_map(|page| {
				Url::chapter_page(manga_key, chapter_key, page)
					.request()
					.ok()
			})
			.collect();
		if requests.is_empty() {
			break;
		}

		let mut end_valid = false;
		for (index, result) in Request::send_all(requests).into_iter().enumerate() {
			let Ok(response) = result else {
				continue;
			};
			let html = response.get_string().unwrap_or_default();
			let Some((page, page_total)) = html::fragment_position(&html) else {
				continue;
			};
			if page > page_total {
				continue; // out-of-range stub
			}
			total = Some(page_total);
			if start + index as i32 == end {
				end_valid = true;
			}
			html::extract_fragment(&html, page, chapter_id, params, fragments, seen_images);
		}

		if !end_valid || end >= MAX_CHAPTER_PAGES {
			break;
		}
		start = end + 1;
	}
	total
}

register_source!(
	Bilinovel,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
