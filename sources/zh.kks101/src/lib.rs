#![no_std]

mod home;
mod html;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::html::Html,
	imports::net::Request,
	imports::std::send_partial_result,
	prelude::*,
};

use html::{ChapterPage as _, MangaPage as _};
use net::Url;

pub const BASE_URL: &str = "https://101kks.com";

struct Kks101;

impl Source for Kks101 {
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
		let response = url.request()?.send()?;

		// A search that uniquely matches one book is 302-redirected by the
		// site straight to its detail page (`/book/{id}.html`) instead of
		// returning a result list. Surface that book as a single result.
		let final_url = response.get_url().unwrap_or_default();
		if final_url.contains("/book/") {
			let Some(key) = book_id_from_url(&final_url) else {
				bail!("unrecognized redirect target: {}", final_url);
			};
			let document = Html::parse(response.get_string()?.as_bytes())?;
			let manga = document
				.manga_from_book_page(key)
				.ok_or_else(|| error!("unique search hit for {} did not parse", final_url))?;
			return Ok(MangaPageResult {
				entries: vec![manga],
				has_next_page: false,
			});
		}

		let document = Html::parse(response.get_string()?.as_bytes())?;
		document.manga_page_result(page)
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
			manga.chapters = Some(fetch_chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let response = Url::chapter(&manga.key, &chapter.key).request()?.send()?;
		let page_html = response.get_string()?;
		if html::is_challenge_page(&page_html) {
			bail!(
				"chapter {} of {}: site returned a Cloudflare challenge page",
				chapter.key,
				manga.key
			);
		}
		let document = Html::parse(page_html.as_bytes())?;
		let content = document
			.select_first("#txtcontent")
			.ok_or_else(|| error!("no #txtcontent on chapter {} of {}", chapter.key, manga.key))?;
		let raw = content.html().unwrap_or_default();
		let text = html::chapter_text(&raw);
		if text.is_empty() {
			bail!("chapter {} of {}: empty content", chapter.key, manga.key);
		}
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl ListingProvider for Kks101 {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let (order, isfull) = match listing.id.as_str() {
			"new" => ("newhot", "0"),
			"popular" => ("weekvisit", "0"),
			"recommended" => ("allvote", "0"),
			"completed" => ("newhot", "1"),
			_ => bail!("Unknown listing: {}", listing.id),
		};
		let url = Url::List {
			order,
			tagid: String::from("0"),
			isfull: String::from(isfull),
			page,
		};
		let document = url.request()?.html()?;
		document.manga_page_result(page)
	}
}

impl ImageRequestProvider for Kks101 {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Kks101 {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split(['?', '#'])
			.next()
			.unwrap_or(&url)
			.rsplit("101kks.com")
			.next()
			.unwrap_or("")
			.trim_start_matches('/');

		let mut parts = path.splitn(3, '/');
		match parts.next() {
			// /book/{id}.html
			Some("book") => {
				let id = parts.next().unwrap_or("").trim_end_matches(".html");
				if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
					return Ok(None);
				}
				Ok(Some(DeepLinkResult::Manga { key: id.into() }))
			}
			// /txt/{book}/{chapter}.html
			Some("txt") => {
				let book = parts.next().unwrap_or("");
				let chapter = parts
					.next()
					.unwrap_or("")
					.trim_end_matches(".html")
					.split('_')
					.next()
					.unwrap_or("");
				if book.is_empty()
					|| chapter.is_empty()
					|| !book.chars().all(|c| c.is_ascii_digit())
					|| !chapter.chars().all(|c| c.is_ascii_digit())
				{
					return Ok(None);
				}
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: book.into(),
					key: chapter.into(),
				}))
			}
			_ => Ok(None),
		}
	}
}

/// Fetch the full chapter list, preferring the AJAX endpoint (served behind
/// a Cloudflare challenge) and falling back to the catalog embedded in the
/// book page, which only lists the first and last chapters.
fn fetch_chapters(manga_key: &str) -> Result<Vec<Chapter>> {
	let ajax_response = Url::catalog_ajax(manga_key).request()?.send()?;
	let ajax_html = ajax_response.get_string().unwrap_or_default();
	if ajax_response.status_code() == 200
		&& !html::is_challenge_page(&ajax_html)
		&& let Ok(document) = Html::parse(ajax_html.as_bytes())
	{
		let chapters = document.chapters()?;
		if !chapters.is_empty() {
			return Ok(chapters);
		}
	}

	// Fallback: the book page's own catalog (first + last chapters only).
	let document = Url::catalog(manga_key).request()?.html()?;
	document.chapters()
}

/// Extract the numeric book id from a `/book/{id}.html` URL.
fn book_id_from_url(url: &str) -> Option<String> {
	let segment = url.trim_end_matches(".html").rsplit('/').next()?;
	segment.parse::<u64>().ok().map(|_| segment.to_string())
}

register_source!(
	Kks101,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
