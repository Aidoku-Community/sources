#![no_std]

mod html;
mod json;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, ImageRequestProvider, Manga, MangaPageResult, Page,
	Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use html::{ChapterPage as _, MangaPage as _, PageList as _};
use json::ApiResponse;
use net::Url;

pub const BASE_URL: &str = "https://www.baozimh.com";

struct Baozimanhua;

impl Source for Baozimanhua {
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

		// API request returns JSON
		if let Url::Filter { .. } = &url {
			let request = url.request()?;
			let json_data = request.data()?;
			let response: ApiResponse = serde_json::from_slice(&json_data)?;
			return Ok(response.into());
		}

		// Search and other requests return HTML
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
			let chapter_page = Url::manga(manga.key.clone()).request()?.html()?;
			manga.chapters = Some(chapter_page.chapters(&manga.key)?);
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

impl ImageRequestProvider for Baozimanhua {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Baozimanhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let url = url.trim_start_matches(BASE_URL);
		let mut splits = url.split('/').skip(1);
		let deep_link_result = match splits.next() {
			Some("comic") => {
				match (splits.next(), splits.next()) {
					// /comic/{manga_id}
					(Some(manga_id), None) => Some(DeepLinkResult::Manga {
						key: manga_id.into(),
					}),
					// /comic/chapter/{manga_id}
					(Some("chapter"), Some(path)) => {
						let parts: Vec<&str> = path.split('_').collect();
						if parts.len() >= 2 {
							let manga_id = parts[0];
							let chapter_id = parts[1].trim_end_matches(".html");
							Some(DeepLinkResult::Chapter {
								manga_key: manga_id.into(),
								key: chapter_id.into(),
							})
						} else {
							None
						}
					}
					_ => None,
				}
			}
			_ => None,
		};
		Ok(deep_link_result)
	}
}

register_source!(Baozimanhua, ImageRequestProvider, DeepLinkHandler);
