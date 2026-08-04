#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, Listing,
	ListingProvider, Manga, MangaPageResult, Page, Result, Source,
	alloc::{borrow::Cow, string::String, vec::Vec},
};

mod helpers;
mod imp;
mod models;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub api_url: Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			api_url: "".into(),
		}
	}
}

pub struct EzManhwa<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for EzManhwa<T> {
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

impl<T: Impl> ListingProvider for EzManhwa<T> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}

impl<T: Impl> Home for EzManhwa<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> DeepLinkHandler for EzManhwa<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
