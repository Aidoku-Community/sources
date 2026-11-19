// ported from https://github.com/keiyoushi/extensions-source/tree/main/lib-multisrc/heancms/src/eu/kanade/tachiyomi/multisrc/heancms
#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, Manga,
	MangaPageResult, Page, Result, Source,
	alloc::{String, Vec, borrow::Cow},
};

mod imp;
mod models;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub api_url: Cow<'static, str>,
	pub latest_sort_ascending: bool,
}

pub struct HeanCms<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for HeanCms<T> {
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

impl<T: Impl> DynamicFilters for HeanCms<T> {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		self.inner.get_dynamic_filters(&self.params)
	}
}

impl<T: Impl> DeepLinkHandler for HeanCms<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
