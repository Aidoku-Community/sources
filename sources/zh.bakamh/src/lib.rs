#![no_std]

mod html;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use html::{ChapterPage as _, MangaPage as _};
use net::Url;

pub const BASE_URL: &str = "https://bakamh.com";

struct Bakamh;

impl Source for Bakamh {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let is_search = query.is_some();
		let url = Url::from_query_or_filters(query.as_deref(), page, &filters)?;
		let html = url.request()?.html()?;
		html.manga_page_result(is_search)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}/manga/{}/", BASE_URL, manga.key);
		let html = Request::get(url)?.html()?;

		if needs_details {
			html.update_details(&mut manga)?;
		}

		if needs_chapters {
			manga.chapters = Some(html.chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		html::get_page_list(&chapter.key)
	}
}

impl ListingProvider for Bakamh {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"latest" | "rating" | "views" | "new-manga" => format!(
				"{}/page/{}/?s&post_type=wp-manga&m_orderby={}",
				BASE_URL, page, listing.id
			),
			_ => bail!("Invalid listing"),
		};
		let html = Request::get(url)?.html()?;
		html.manga_page_result(true)
	}
}

impl ImageRequestProvider for Bakamh {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Bakamh {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.trim_start_matches(BASE_URL).trim_start_matches('/');
		let mut parts = path.split('/').filter(|s| !s.is_empty());
		let result = match (parts.next(), parts.next(), parts.next()) {
			(Some("manga"), Some(manga_key), None) => Some(DeepLinkResult::Manga {
				key: manga_key.into(),
			}),
			(Some("manga"), Some(manga_key), Some(chapter_slug)) => Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: format!("/manga/{}/{}/", manga_key, chapter_slug),
			}),
			_ => None,
		};
		Ok(result)
	}
}

register_source!(
	Bakamh,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
