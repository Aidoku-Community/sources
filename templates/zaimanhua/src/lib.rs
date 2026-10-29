#![no_std]

use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, DynamicSettings,
	Filter, FilterValue, Home, HomeLayout, Listing, ListingProvider, Manga, MangaPageResult,
	NotificationHandler, Page, Result, Setting, Source,
	alloc::{String, Vec},
};

pub const V4_API_URL: &str = "https://v4api.zaimanhua.com/app/v1";
const ACCOUNT_API_URL: &str = "https://account-api.zaimanhua.com/v1";
const NEWS_URL: &str = "https://news.zaimanhua.com";
pub const PAGE_SIZE: i32 = 20;

pub mod helpers;
pub mod home;
mod imp;
pub mod models;
pub mod net;
pub mod settings;

pub use imp::Impl;

#[derive(Clone, Copy)]
pub enum Kind {
	Comic,
	Novel,
}

impl Kind {
	fn path(self) -> &'static str {
		match self {
			Self::Comic => "comic",
			Self::Novel => "novel",
		}
	}
}

pub struct Params {
	pub kind: Kind,
}

pub struct ZaimanhuaTemplate<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for ZaimanhuaTemplate<T> {
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

impl<T: Impl> ListingProvider for ZaimanhuaTemplate<T> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}

impl<T: Impl> Home for ZaimanhuaTemplate<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> DeepLinkHandler for ZaimanhuaTemplate<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, &url)
	}
}

impl<T: Impl> BasicLoginHandler for ZaimanhuaTemplate<T> {
	fn handle_basic_login(&self, key: String, username: String, password: String) -> Result<bool> {
		self.inner.handle_basic_login(key, username, password)
	}
}

impl<T: Impl> NotificationHandler for ZaimanhuaTemplate<T> {
	fn handle_notification(&self, notification: String) {
		self.inner.handle_notification(notification)
	}
}

impl<T: Impl> DynamicSettings for ZaimanhuaTemplate<T> {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		self.inner.get_dynamic_settings()
	}
}

impl<T: Impl> DynamicFilters for ZaimanhuaTemplate<T> {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		self.inner.get_dynamic_filters(&self.params)
	}
}
