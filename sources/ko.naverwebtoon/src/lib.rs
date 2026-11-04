#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, ImageRequestProvider, Manga,
	MangaPageResult, NotificationHandler, Page, PageContext, Source, WebLoginHandler,
	alloc::{String, Vec},
	imports::error::Result,
	imports::net::{Request, TimeUnit, set_rate_limit},
	register_source,
};

mod auth;
mod helper;
mod parser;

pub struct NaverWebtoon;

impl Source for NaverWebtoon {
	fn new() -> Self {
		set_rate_limit(5, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		parser::parse_search_manga_list(query, page, filters)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		parser::parse_manga_update(manga, needs_details, needs_chapters)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		parser::parse_page_list(&manga.key, &chapter.key)
	}
}

impl ImageRequestProvider for NaverWebtoon {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		parser::get_image_request(url)
	}
}

impl DeepLinkHandler for NaverWebtoon {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		parser::parse_deep_link(&url)
	}
}

/// Called by Aidoku whenever cookies are updated in the in-app webview login flow.
/// Returning Ok(true) signals that authentication is complete and the webview can close.
impl WebLoginHandler for NaverWebtoon {
	fn handle_web_login(&self, _key: String, cookies: HashMap<String, String>) -> Result<bool> {
		auth::handle_login(cookies)
	}
}

/// Called by Aidoku when the user taps "로그아웃" in source settings (notification key: "login").
impl NotificationHandler for NaverWebtoon {
	fn handle_notification(&self, notification: String) {
		if notification == "login" {
			auth::logout();
		}
	}
}

register_source!(
	NaverWebtoon,
	ImageRequestProvider,
	DeepLinkHandler,
	WebLoginHandler,
	NotificationHandler
);
