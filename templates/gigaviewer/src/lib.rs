#![no_std]
use aidoku::{
	alloc::{borrow::Cow, String, Vec},
	imports::canvas::ImageRef,
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout,
	ImageResponse, Listing, ListingProvider, Manga, MangaPageResult, NotificationHandler, Page,
	PageContext, PageImageProcessor, Result, Source,
};

mod auth;
mod imp;
mod models;
pub mod parser;

pub use auth::AuthedRequest;
pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub cdn_url: Cow<'static, str>,
	pub is_paginated: bool,
	pub popular_item_selector: Cow<'static, str>,
	pub chapter_list_selector: Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			cdn_url: "".into(),
			is_paginated: false,
			popular_item_selector: "ul.series-list li a".into(),
			chapter_list_selector: "li.episode".into(),
		}
	}
}

pub struct GigaViewer<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for GigaViewer<T> {
	fn new() -> Self {
		let inner = T::new();
		let params = inner.params();
		Self { inner, params }
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		self.inner
			.get_search_manga_list(&self.params, query, page, filters)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		self.inner
			.get_manga_update(&self.params, manga, needs_details, needs_chapters)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		self.inner.get_page_list(&self.params, manga, chapter)
	}
}

impl<T: Impl> ListingProvider for GigaViewer<T> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}

impl<T: Impl> PageImageProcessor for GigaViewer<T> {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		self.inner
			.process_page_image(&self.params, response, context)
	}
}

impl<T: Impl> Home for GigaViewer<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> BasicLoginHandler for GigaViewer<T> {
	fn handle_basic_login(&self, key: String, username: String, password: String) -> Result<bool> {
		self.inner
			.handle_basic_login(&self.params, key, username, password)
	}
}

impl<T: Impl> NotificationHandler for GigaViewer<T> {
	fn handle_notification(&self, notification: String) {
		self.inner.handle_notification(&self.params, notification);
	}
}

impl<T: Impl> DeepLinkHandler for GigaViewer<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
