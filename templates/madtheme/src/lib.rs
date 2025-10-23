#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	Manga, MangaPageResult, Page, PageContext, Result, Source, Viewer,
	alloc::{String, Vec, borrow::Cow},
	imports::net::Request,
};

mod helpers;
mod imp;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub use_slug_search: bool,
	pub default_rating: ContentRating,
	pub default_viewer: Viewer,
	pub date_format: Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			use_slug_search: false,
			default_rating: ContentRating::default(),
			default_viewer: Viewer::default(),
			date_format: "MMM dd, yyyy".into(),
		}
	}
}

pub struct MadTheme<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for MadTheme<T> {
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

impl<T: Impl> ImageRequestProvider for MadTheme<T> {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		self.inner.get_image_request(&self.params, url, context)
	}
}

impl<T: Impl> DeepLinkHandler for MadTheme<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
