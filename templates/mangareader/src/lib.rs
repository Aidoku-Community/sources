#![no_std]
use aidoku::{
	alloc::{borrow::Cow, String, Vec},
	imports::{canvas::ImageRef, net::Request},
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, ImageRequestProvider,
	ImageResponse, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContext,
	PageImageProcessor, Result, Source,
};

mod helper;
mod imp;
pub mod parser;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub search_path: Cow<'static, str>,
	pub search_param: Cow<'static, str>,
	pub page_param: Cow<'static, str>,
	pub page_selector: Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			search_path: "/search".into(),
			search_param: "keyword".into(),
			page_param: "page".into(),
			page_selector: ".container-reader-chapter > div > img".into(),
		}
	}
}

pub struct MangaReader<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for MangaReader<T> {
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

impl<T: Impl> ListingProvider for MangaReader<T> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}

impl<T: Impl> Home for MangaReader<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> ImageRequestProvider for MangaReader<T> {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		self.inner.get_image_request(&self.params, url, context)
	}
}

impl<T: Impl> PageImageProcessor for MangaReader<T> {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		self.inner
			.process_page_image(&self.params, response, context)
	}
}

impl<T: Impl> DeepLinkHandler for MangaReader<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
