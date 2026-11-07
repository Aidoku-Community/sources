#![no_std]

mod home;
mod html;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::{html::Document, net::Request, std::sleep},
	prelude::*,
};

use html::{ChapterPage as _, MangaPage as _};
use net::Url;

pub const BASE_URL: &str = "https://www.suduguu.com";

/// Hard cap on paginated body pages, guarding against malformed page chains.
const MAX_PAGES: i32 = 50;

/// Number of request attempts per paginated chapter page (initial + retries).
const MAX_ATTEMPTS: u32 = 3;

/// Fetch one paginated chapter page. On failure (network error or rate
/// limit) backs off with an increasing delay (2s, then 4s) and retries, up
/// to `MAX_ATTEMPTS` attempts. Returns `None` after all attempts fail; the
/// caller turns that into an error rather than serving partial content.
fn fetch_chapter_page(book: &str, chapter: &str, page: i32) -> Option<Document> {
	let mut attempt: u32 = 0;
	loop {
		let result = Url::chapter_page(book, chapter, page)
			.request()
			.and_then(|r| Ok(r.html()?));
		match result {
			Ok(document) => return Some(document),
			Err(_) => {
				attempt += 1;
				if attempt >= MAX_ATTEMPTS {
					return None;
				}
				// Increasing backoff: 2s after the first failure, 4s after
				// the second (aidoku's sleep is whole-second only).
				sleep(2 * attempt as i32);
			}
		}
	}
}

/// Extract the body text of one chapter page. `None` when the `.con`
/// container is missing; an empty string means the body is empty (a common
/// rate-limit symptom, retried by the caller).
fn extract_page_text(document: &Document, page_no: i32) -> Option<String> {
	let content = document.select_first(".con")?;
	let raw = content.html().unwrap_or_default();
	let title = if page_no == 1 {
		html::chapter_title(document).unwrap_or_default()
	} else {
		String::new()
	};
	let text = html::strip_leading_title(&html::chapter_text(&raw), &title);
	Some(text)
}

struct Suduguu;

impl Source for Suduguu {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = match query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
			Some(q) => Url::Search {
				query: q.to_string(),
			},
			None => Url::from_filters(filters, page),
		};
		let document = url.request()?.html()?;
		document.manga_page_result(page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// The book page embeds the complete chapter list, so a single
		// request covers details and chapters alike.
		let document = Url::manga(&manga.key).request()?.html()?;

		if needs_details {
			document.update_details(&mut manga)?;
		}

		if needs_chapters {
			let chapters = document.chapters()?;
			if chapters.is_empty() {
				bail!("book {}: no chapters found", manga.key);
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// Long chapters are paginated as `/{book}/{ch}-{n}.html`; the first
		// page carries the whole chain via the "下一页" link in `.prenext`.
		// Pages are fetched serially with a 1s/2s jittered throttle between
		// requests (the site rate-limits bursts) and increasing-backoff
		// retries, mirroring so-novel's crawl strategy for this site.
		let mut parts: Vec<String> = Vec::new();
		let mut page_no: i32 = 1;
		loop {
			if page_no > 1 {
				// Alternate 1s/2s between pages to approximate a jittered
				// pace (aidoku's sleep is whole-second only).
				sleep(if page_no % 2 == 0 { 2 } else { 1 });
			}
			// A failed page must not silently truncate the chapter: an
			// incomplete body would be marked read without indication.
			let mut document = match fetch_chapter_page(&manga.key, &chapter.key, page_no) {
				Some(doc) => doc,
				None => bail!(
					"chapter {} of {}: failed to fetch page {}",
					chapter.key,
					manga.key,
					page_no
				),
			};
			let mut text = match extract_page_text(&document, page_no) {
				Some(t) => t,
				None => bail!(
					"chapter {} of {}: no content container on page {}",
					chapter.key,
					manga.key,
					page_no
				),
			};
			// An empty body is a rate-limit symptom (so-novel asserts
			// non-empty and retries the chapter); retry once after a 2s
			// backoff before giving up on this page.
			if text.is_empty() {
				sleep(2);
				if let Some(retried) = fetch_chapter_page(&manga.key, &chapter.key, page_no) {
					document = retried;
					if let Some(t) = extract_page_text(&document, page_no) {
						text = t;
					}
				}
				if text.is_empty() {
					bail!(
						"chapter {} of {}: empty content on page {}",
						chapter.key,
						manga.key,
						page_no
					);
				}
			}
			parts.push(text);
			if !html::has_next_page(&document, &manga.key, &chapter.key, page_no) {
				break;
			}
			page_no += 1;
			if page_no > MAX_PAGES {
				break;
			}
		}
		if parts.is_empty() {
			bail!("chapter {} of {}: empty content", chapter.key, manga.key);
		}
		Ok(vec![Page {
			content: PageContent::text(parts.join("\n\n")),
			..Default::default()
		}])
	}
}

impl ListingProvider for Suduguu {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let category = match listing.id.as_str() {
			"paihang" => "paihang",
			"wanjie" => "wanjie",
			"zuixin" => "zuixin",
			_ => bail!("Unknown listing: {}", listing.id),
		};
		let url = Url::List {
			category: category.into(),
			page,
		};
		let document = url.request()?.html()?;
		document.manga_page_result(page)
	}
}

impl ImageRequestProvider for Suduguu {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Suduguu {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split(['?', '#'])
			.next()
			.unwrap_or(&url)
			.rsplit("suduguu.com")
			.next()
			.unwrap_or("")
			.trim_matches('/');

		let mut parts = path.splitn(2, '/');
		match (parts.next(), parts.next()) {
			// `/{bookId}/`
			(Some(book), None) if is_numeric(book) => {
				Ok(Some(DeepLinkResult::Manga { key: book.into() }))
			}
			// `/{bookId}/{chapterId}.html`
			(Some(book), Some(chapter)) => {
				let chapter = chapter.trim_end_matches(".html");
				if is_numeric(book) && is_numeric(chapter) {
					Ok(Some(DeepLinkResult::Chapter {
						manga_key: book.into(),
						key: chapter.into(),
					}))
				} else {
					Ok(None)
				}
			}
			_ => Ok(None),
		}
	}
}

fn is_numeric(s: &str) -> bool {
	!s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

register_source!(
	Suduguu,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
