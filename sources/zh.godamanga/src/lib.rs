#![no_std]

mod html;
mod json;
mod net;

use aidoku::{
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
	Chapter, DeepLinkHandler, DeepLinkResult, ImageRequestProvider, Listing, ListingProvider,
	Manga, MangaPageResult, Page, Result, Source,
};
use html::MangaPage as _;
use net::Url;

pub const BASE_URL: &str = "https://godamh.com";
pub const API_URL: &str = "https://api-get-v3.mgsearcher.com";
pub const IMG_URL: &str = "https://f40-1-4.g-mh.online";

fn handle_cover_url(url: String) -> String {
	if url.contains("url=") {
		url.split("url=")
			.last()
			.unwrap_or(&url)
			.replace("%3A", ":")
			.replace("%2F", "/")
			.replace("&w=250&q=60", "")
	} else {
		url
	}
}

struct Godamanga;

impl Source for Godamanga {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<aidoku::FilterValue>,
	) -> Result<MangaPageResult> {
		let url = Url::from_query_or_filters(query.as_deref(), page, &filters)?;
		let html = url.request()?.html()?;
		html.manga_page_result()
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let manga_page = Url::manga(manga.key.clone()).request()?.html()?;
			manga_page.update_details(&mut manga)?;
		}

		if needs_chapters {
			manga.chapters = Some(json::chapter_list::ChapterList::get_chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		json::page_list::PageList::get_pages(manga.key, chapter.key)
	}
}

impl ImageRequestProvider for Godamanga {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Godamanga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let url = url.trim_start_matches(BASE_URL);
		let mut splits = url.split('/').skip(1);
		let deep_link_result = match splits.next() {
			Some("manga") => match (splits.next(), splits.next()) {
				(Some(manga_id), None) => Some(DeepLinkResult::Manga {
					key: manga_id.into(),
				}),
				(Some(manga_id), Some(chapter_id)) => Some(DeepLinkResult::Chapter {
					manga_key: manga_id.into(),
					key: chapter_id.into(),
				}),
				_ => None,
			},
			_ => None,
		};
		Ok(deep_link_result)
	}
}

impl ListingProvider for Godamanga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"hots" => format!("{}/hots/page/{}", BASE_URL, page),
			"dayup" => format!("{}/dayup/page/{}", BASE_URL, page),
			"newss" => format!("{}/newss/page/{}", BASE_URL, page),
			_ => bail!("Invalid listing"),
		};

		let html = Request::get(url)?.header("Origin", BASE_URL).html()?;

		html.manga_page_result()
	}
}

register_source!(
	Godamanga,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
