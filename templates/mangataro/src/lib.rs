#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, DynamicSettings, Filter, FilterValue,
	Home, HomeLayout, ImageRequestProvider, Manga, MangaPageResult, Page, PageContext, Result,
	Setting, Source,
	alloc::{String, Vec, borrow::Cow},
	imports::net::Request,
};

mod api;
mod helpers;
mod home;
mod imp;
mod markdown;
mod models;
mod settings;

pub use home::Section;
pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub sections: &'static [Section],
}

pub struct MangaTaro<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for MangaTaro<T> {
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

impl<T: Impl> Home for MangaTaro<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> DynamicFilters for MangaTaro<T> {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		self.inner.get_dynamic_filters(&self.params)
	}
}

impl<T: Impl> DynamicSettings for MangaTaro<T> {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		self.inner.get_dynamic_settings(&self.params)
	}
}

impl<T: Impl> DeepLinkHandler for MangaTaro<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}

impl<T: Impl> ImageRequestProvider for MangaTaro<T> {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", self.params.base_url.as_ref()))
	}
}
