#![no_std]

mod html;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, ImageRequestProvider, Manga, MangaPageResult, Page,
	Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use html::{ChapterPage as _, MangaPage as _, PageList as _};
use net::Url;

pub const BASE_URL: &str = "https://www.bilimanga.net";

struct Bilimanga;

impl Source for Bilimanga {
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
			let chapter_list_page = Url::chapter_list(manga.key.clone()).request()?.html()?;
			manga.chapters = Some(chapter_list_page.chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Url::chapter(manga.key, chapter.key)
			.request()?
			.html()?
			.pages()
	}
}

impl ImageRequestProvider for Bilimanga {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Bilimanga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let url = url.trim_start_matches(BASE_URL);
		let mut splits = url.split('/').skip(1);
		let deep_link_result = match splits.next() {
			Some("detail") => match splits.next() {
				Some(id) => {
					let id = id.trim_end_matches(".html");
					Some(DeepLinkResult::Manga { key: id.into() })
				}
				_ => None,
			},
			Some("read") => match (splits.next(), splits.next(), splits.next()) {
				(Some(manga_id), Some(chapter_id), None) => {
					let chapter_id = chapter_id.trim_end_matches(".html");
					Some(DeepLinkResult::Chapter {
						manga_key: manga_id.into(),
						key: chapter_id.into(),
					})
				}
				_ => None,
			},
			_ => None,
		};
		Ok(deep_link_result)
	}
}

register_source!(Bilimanga, ImageRequestProvider, DeepLinkHandler);
