#![no_std]
use aidoku::{
	alloc::{borrow::Cow, String, Vec},
	imports::net::Request,
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, ImageRequestProvider,
	Manga, MangaPageResult, Page, PageContext, Result, Source,
};

mod helpers;
mod imp;
pub mod models;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub api_url: Option<Cow<'static, str>>,
	// if the post endpoint require slugs instead of ids
	pub use_slug_series_keys: bool,
	// the post endpoint doesn't contain all keys for the chapter objects
	pub fetch_full_chapter_list: bool,
	pub get_sort_value: fn(i32) -> Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			api_url: None,
			use_slug_series_keys: false,
			fetch_full_chapter_list: false,
			get_sort_value: |_| "".into(),
		}
	}
}

impl Params {
	#[inline]
	pub fn get_api_url(&self) -> Cow<'static, str> {
		self.api_url
			.clone()
			.unwrap_or_else(|| self.base_url.clone())
	}
}

pub struct Iken<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for Iken<T> {
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

impl<T: Impl> Home for Iken<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> ImageRequestProvider for Iken<T> {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		self.inner.get_image_request(&self.params, url, context)
	}
}

impl<T: Impl> DeepLinkHandler for Iken<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
