#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, ImageRequestProvider,
	Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, borrow::Cow},
	imports::net::Request,
};

mod helpers;
pub mod home;
mod imp;
mod markdown;
mod models;
mod settings;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub api_url: Cow<'static, str>,
	pub home_slider_selector: Cow<'static, str>,
	pub home_section_titles: &'static [&'static str],
	pub sorts: &'static [&'static str],
	pub page_size: i32,
	pub banner_width: Option<i32>,
	pub banner_height: Option<i32>,
	pub sort_pages_by_filename: bool,
	pub new_releases_title: &'static str,
	pub novels_title: &'static str,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			api_url: "".into(),
			new_releases_title: "New Releases",
			novels_title: "Novels",
			home_slider_selector: ".main-banner-slider .swiper-slide, .mainSlider .swiper-slide, .cinematic-slider-5 .swiper-slide".into(),
			page_size: 18,
			banner_width: None,
			banner_height: None,
			sort_pages_by_filename: false,
			sorts: &["lastChapterAddedAt", "totalViews", "createdAt", "chaptersCount", "postTitle"],
			home_section_titles: &["Popular Today", "Latest Releases", "Most Popular"],
		}
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
