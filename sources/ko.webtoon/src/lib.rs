#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, Home, HomeLayout,
	ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, NotificationHandler,
	Page, PageContext, Source, WebLoginHandler,
	alloc::{String, Vec},
	imports::error::Result,
	imports::net::Request,
	register_source,
};

mod auth;
mod helper;
mod parser;

pub struct WebtoonKR;

impl Source for WebtoonKR {
	fn new() -> Self {
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

impl Home for WebtoonKR {
	fn get_home(&self) -> Result<HomeLayout> {
		parser::parse_home()
	}
}

impl ListingProvider for WebtoonKR {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		parser::parse_manga_listing(listing, page)
	}
}

impl ImageRequestProvider for WebtoonKR {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		parser::parse_image_request(url, context)
	}
}

impl DeepLinkHandler for WebtoonKR {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		parser::parse_deep_link(url)
	}
}

impl WebLoginHandler for WebtoonKR {
	fn handle_web_login(&self, _key: String, cookies: HashMap<String, String>) -> Result<bool> {
		auth::handle_login(cookies)
	}
}

impl NotificationHandler for WebtoonKR {
	fn handle_notification(&self, notification: String) {
		if notification == "login" || notification == "logout" {
			auth::logout();
		}
	}
}

register_source!(
	WebtoonKR,
	Home,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler,
	WebLoginHandler,
	NotificationHandler
);
