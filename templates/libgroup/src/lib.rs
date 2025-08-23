#![no_std]
use aidoku::{
	AlternateCoverProvider, Chapter, FilterValue, Home, HomeLayout, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, MigrationHandler, NotificationHandler, Page,
	PageContext, Result, Source,
	alloc::{String, Vec, borrow::Cow},
	imports::net::Request,
};

mod auth;
mod cdn;
mod chapters;
mod context;
mod endpoints;
mod filters;
mod home;
mod imp;
mod models;
mod settings;

pub use imp::Impl;

pub struct Params {
	pub site_id: Cow<'static, u8>,
}

pub struct LibGroup<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for LibGroup<T> {
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

impl<T: Impl> ListingProvider for LibGroup<T> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}
impl<T: Impl> Home for LibGroup<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> ImageRequestProvider for LibGroup<T> {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		self.inner.get_image_request(&self.params, url, context)
	}
}

impl<T: Impl> AlternateCoverProvider for LibGroup<T> {
	fn get_alternate_covers(&self, manga: Manga) -> Result<Vec<String>> {
		self.inner.get_alternate_covers(&self.params, manga)
	}
}

impl<T: Impl> MigrationHandler for LibGroup<T> {
	fn handle_manga_migration(&self, key: String) -> Result<String> {
		self.inner.handle_manga_migration(&self.params, key)
	}

	fn handle_chapter_migration(&self, manga_key: String, chapter_key: String) -> Result<String> {
		self.inner
			.handle_chapter_migration(&self.params, manga_key, chapter_key)
	}
}

impl<T: Impl> NotificationHandler for LibGroup<T> {
	fn handle_notification(&self, notification: String) {
		self.inner.handle_notification(&self.params, notification);
	}
}
