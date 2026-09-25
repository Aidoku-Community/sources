#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicListings, FilterValue, Home, HomeLayout,
	ImageResponse, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContext,
	PageImageProcessor, Result, Source,
	alloc::{String, Vec, borrow::Cow},
	imports::canvas::ImageRef,
};

mod endpoints;
mod home;
mod imp;
mod json;
mod models;
mod request;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub domain: Cow<'static, str>,
	pub service_name: Cow<'static, str>,
	pub key_decryption: Cow<'static, str>,
}

pub struct OtakuOvh<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for OtakuOvh<T> {
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

impl<T: Impl> ListingProvider for OtakuOvh<T> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}

impl<T: Impl> Home for OtakuOvh<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> DeepLinkHandler for OtakuOvh<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}

impl<T: Impl> PageImageProcessor for OtakuOvh<T> {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		self.inner.process_page_image(response, context)
	}
}

impl<T: Impl> DynamicListings for OtakuOvh<T> {
	fn get_dynamic_listings(&self) -> Result<Vec<Listing>> {
		self.inner.get_dynamic_listings()
	}
}

