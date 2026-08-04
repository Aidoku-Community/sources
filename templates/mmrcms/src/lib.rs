#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, Home,
	HomeLayout, ImageRequestProvider, ListingProvider, Manga, MangaPageResult, Page, PageContext,
	Result, Source,
	alloc::{String, Vec, borrow::Cow},
	imports::net::Request,
};

pub mod helpers;
mod imp;
mod models;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub item_path: Cow<'static, str>,
	pub supports_advanced_search: bool,
	pub details_title_selector: Cow<'static, str>,
	pub manga_list_selector: Cow<'static, str>,
	pub manga_list_next_page_selector: Cow<'static, str>,
	pub chapter_list_selector: Cow<'static, str>,
	pub chapter_name_prefix: Cow<'static, str>,
	pub chapter_string: Cow<'static, str>,
	pub date_format: Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			item_path: "manga".into(),
			supports_advanced_search: true,
			details_title_selector: ".listmanga-header, .widget-title".into(),
			manga_list_selector: "div.media".into(),
			manga_list_next_page_selector: ".pagination a[rel=next]".into(),
			chapter_list_selector: "ul.chapters > li:not(.btn)".into(),
			chapter_name_prefix: "".into(),
			chapter_string: "Chapter".into(),
			date_format: "d MMM. yyyy".into(),
		}
	}
}

pub struct MMRCMS<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for MMRCMS<T> {
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

impl<T: Impl> ListingProvider for MMRCMS<T> {
	fn get_manga_list(&self, listing: aidoku::Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}

impl<T: Impl> DynamicFilters for MMRCMS<T> {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		self.inner.get_dynamic_filters(&self.params)
	}
}

impl<T: Impl> ImageRequestProvider for MMRCMS<T> {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		self.inner.get_image_request(&self.params, url, context)
	}
}

impl<T: Impl> Home for MMRCMS<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> DeepLinkHandler for MMRCMS<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
