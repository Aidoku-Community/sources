#![cfg_attr(not(test), no_std)]

mod html;
mod json;
mod net;

use aidoku::imports::net::Request;
use aidoku::imports::std::send_partial_result;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec},
	prelude::*,
};

use html::RumanhuaDetailsHtml as _;
use json::parse_images_from_html;
use net::{get_chapter_url, get_request, get_search_url};

struct Rumanhua;

impl Source for Rumanhua {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = get_search_url(query, page, filters);
		let html = get_request(&url)?.html()?;
		html::parse_manga_list(&html)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = net::get_absolute_url(&format!("/news/{}", manga.key));
		let html = get_request(&url)?.html()?;

		if needs_details {
			html.update_details(&mut manga)?;

			if needs_chapters {
				send_partial_result(&manga);
			} else {
				return Ok(manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(html.get_chapters()?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = get_chapter_url(&chapter);
		let html_str = get_request(&url)?.string()?;

		let images = parse_images_from_html(&html_str)?;
		let mut pages = Vec::new();
		for img_url in images {
			pages.push(Page {
				content: PageContent::url(img_url),
				..Default::default()
			});
		}

		Ok(pages)
	}
}

impl ImageRequestProvider for Rumanhua {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		get_request(&url)
	}
}

impl DeepLinkHandler for Rumanhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		html::handle_deep_link(url)
	}
}

register_source!(Rumanhua, ImageRequestProvider, DeepLinkHandler);
