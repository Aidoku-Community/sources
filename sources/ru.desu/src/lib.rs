#![no_std]
extern crate alloc;

mod helpers;
mod models;
mod settings;

use crate::helpers::{apply_headers, fetch_by_id, fetch_chapter, get_base_url, search};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
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
		// 3 req per 1 sec: https://desu.uno/help/api/
		set_rate_limit(3, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		search(query, page, filters).map(|r| MangaPageResult {
			has_next_page: r.len() >= helpers::PAGE_SIZE,
			entries: r.into_iter().map(|m| m.to_slim_item()).collect(),
		})
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		_needs_details: bool,
		_needs_chapters: bool,
	) -> Result<Manga> {
		fetch_by_id(manga.key.as_str()).map(Manga::from)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let pages = fetch_chapter(manga.key.as_str(), chapter.key.as_str())
			.and_then(|s| {
				s.pages
					.and_then(|x| x.list)
					.ok_or(error!("Chapter {} not found", chapter.key))
			})?
			.into_iter()
			.filter_map(|p| {
				p.img.map(|u| Page {
					content: PageContent::Url(u, None),
					..Page::default()
				})
			})
			.collect();

		Ok(pages)
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
