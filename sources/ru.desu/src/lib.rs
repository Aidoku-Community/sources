#![no_std]
extern crate alloc;

mod helpers;
mod models;
mod settings;

use crate::helpers::{
	apply_headers, fetch_by_id, fetch_chapter_pages, fetch_chapters, get_base_url, search,
};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::imports::std::send_partial_result;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec},
	prelude::*,
};
use alloc::string::ToString;

struct Desu;

impl Source for Desu {
	fn new() -> Self {
		set_rate_limit(3, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let result = search(query, page, filters)?;
		Ok(MangaPageResult {
			has_next_page: result.has_next_page,
			entries: result
				.entries
				.into_iter()
				.map(|m| m.into_manga(None, true, false))
				.collect(),
		})
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut item = if needs_details {
			fetch_by_id(manga.key.as_str())?.into_manga(Some(manga), false, true)
		} else {
			manga
		};

		if needs_chapters {
			if needs_details {
				send_partial_result(&item);
			}
			item.chapters = Some(
				fetch_chapters(item.key.as_str())?
					.into_iter()
					.map(Chapter::from)
					.collect(),
			);
		}

		Ok(item)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Ok(
			fetch_chapter_pages(manga.key.as_str(), chapter.key.as_str())?
				.into_iter()
				.map(|url| Page {
					content: PageContent::url(url),
					..Page::default()
				})
				.collect(),
		)
	}
}

impl DeepLinkHandler for Desu {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(get_base_url().as_str()) else {
			return Ok(None);
		};

		let manga_id = path
			.split('/')
			.skip_while(|&s| s == "manga" || s == "api")
			.find(|s| s.contains('.'))
			.and_then(|s| s.rsplit_once('.').map(|(_, id)| id))
			.or_else(|| {
				path.split('/')
					.find(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
			})
			.ok_or(error!("Invalid URL"))?;

		Ok(Some(DeepLinkResult::Manga {
			key: manga_id.to_string(),
		}))
	}
}

impl ImageRequestProvider for Desu {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(apply_headers(Request::get(url)?))
	}
}

register_source!(Desu, DeepLinkHandler, ImageRequestProvider);
