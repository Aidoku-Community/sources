#![no_std]
use aidoku::{
	alloc::{borrow::Cow, String, Vec},
	imports::net::Request,
	prelude::*,
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, Home,
	HomeLayout, ImageRequestProvider, ListingProvider, Manga, MangaPageResult, MigrationHandler,
	Page, PageContext, Result, Source, Viewer,
};

mod crypto;
pub mod helpers;
mod imp;
mod models;

pub use imp::Impl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadMoreStrategy {
	Always,
	Never,
	AutoDetect,
}

pub struct Params {
	pub base_url: Cow<'static, str>,
	// the path used in the URL for the manga pages
	// necessary for id migration
	pub source_path: Cow<'static, str>,
	// if the source uses the new ajax endpoint for chapters instead of admin-ajax.php
	pub use_new_chapter_endpoint: bool,
	// if image urls are stored in a style attribute and used as a background-image
	pub use_style_images: bool,
	// if the source uses "madara_load_more" to load manga (has "load more" instead of next/prev pages)
	pub use_load_more_request: LoadMoreStrategy,
	// attempts to remove non-manga items from search load more requests
	// disable if the source incorrectly labels entry types
	pub filter_non_manga_items: bool,
	// the viewer to default to if no type was identified
	pub default_viewer: Viewer,
	pub datetime_format: Cow<'static, str>,
	pub datetime_locale: Cow<'static, str>,
	pub datetime_timezone: Cow<'static, str>,
	// the endpoint containing genre checkboxes (typically the search page)
	pub genre_endpoint: Cow<'static, str>,
	// get the path for the search endpoint with a given page number
	pub search_page: fn(i32) -> Cow<'static, str>,
	pub search_manga_selector: Cow<'static, str>,
	pub search_manga_url_selector: Cow<'static, str>,
	pub search_manga_title_selector: Cow<'static, str>,
	pub search_manga_cover_selector: Cow<'static, str>,
	pub details_title_selector: Cow<'static, str>,
	pub details_cover_selector: Cow<'static, str>,
	pub details_author_selector: Cow<'static, str>,
	pub details_artist_selector: Cow<'static, str>,
	pub details_description_selector: Cow<'static, str>,
	pub details_tag_selector: Cow<'static, str>,
	pub details_status_selector: Cow<'static, str>,
	pub details_type_selector: Cow<'static, str>,
	pub chapter_selector: Cow<'static, str>,
	pub chapter_url_selector: Cow<'static, str>,
	pub chapter_title_selector: Cow<'static, str>,
	pub chapter_date_selector: Cow<'static, str>,
	pub chapter_thumbnail_selector: Cow<'static, str>,
	pub page_list_selector: Cow<'static, str>,
	pub chapter_protector_selector: Cow<'static, str>,
	pub chapter_protector_password_prefix: Cow<'static, str>,
	pub chapter_protector_data_prefix: Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			source_path: "manga".into(),
			use_new_chapter_endpoint: false,
			use_style_images: false,
			use_load_more_request: LoadMoreStrategy::AutoDetect,
			filter_non_manga_items: true,
			default_viewer: Viewer::Unknown,
			datetime_format: "MMMM dd, yyyy".into(),
			datetime_locale: "en_US_POSIX".into(),
			datetime_timezone: "current".into(),
			genre_endpoint: "/?s=genre&post_type=wp-manga".into(),
			search_page: |page| {
				if page == 1 {
					"".into()
				} else {
					format!("page/{page}/").into()
				}
			},
			search_manga_selector: "div.c-tabs-item__content , .manga__item".into(),
			search_manga_url_selector: "div.post-title a".into(),
			search_manga_title_selector: "div.post-title a".into(),
			search_manga_cover_selector: "img".into(),
			details_title_selector: "div.post-title h3, div.post-title h1, #manga-title > h1"
				.into(),
			details_cover_selector: "div.summary_image img".into(),
			details_author_selector: "div.author-content > a, div.manga-authors > a".into(),
			details_artist_selector: "div.artist-content > a".into(),
			details_description_selector: "div.description-summary div.summary__content, \
										   div.summary_content div.post-content_item > h5 + div, \
										   div.summary_content div.manga-excerpt"
				.into(),
			details_tag_selector: "div.genres-content a".into(),
			details_status_selector: "div.summary-heading:contains(Status) + div".into(),
			details_type_selector: "div.post-content_item:contains(Type) div.summary-content"
				.into(),
			chapter_selector: "li.wp-manga-chapter".into(),
			chapter_url_selector: "a".into(),
			chapter_title_selector: "a".into(),
			chapter_date_selector: "span.chapter-release-date".into(),
			chapter_thumbnail_selector: "".into(),
			page_list_selector: "div.page-break, li.blocks-gallery-item, \
								 .reading-content .text-left:not(:has(.blocks-gallery-item)) img"
				.into(),
			chapter_protector_selector: "#chapter-protector-data".into(),
			chapter_protector_password_prefix: "wpmangaprotectornonce='".into(),
			chapter_protector_data_prefix: "chapter_data='".into(),
		}
	}
}

pub struct Madara<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for Madara<T> {
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

impl<T: Impl> ListingProvider for Madara<T> {
	fn get_manga_list(&self, listing: aidoku::Listing, page: i32) -> Result<MangaPageResult> {
		self.inner.get_manga_list(&self.params, listing, page)
	}
}

impl<T: Impl> Home for Madara<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> DynamicFilters for Madara<T> {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		self.inner.get_dynamic_filters(&self.params)
	}
}

impl<T: Impl> ImageRequestProvider for Madara<T> {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		self.inner.get_image_request(&self.params, url, context)
	}
}

impl<T: Impl> DeepLinkHandler for Madara<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}

impl<T: Impl> MigrationHandler for Madara<T> {
	fn handle_manga_migration(&self, key: String) -> Result<String> {
		self.inner.handle_id_migration(&self.params, key)
	}

	fn handle_chapter_migration(&self, _manga_key: String, chapter_key: String) -> Result<String> {
		self.inner.handle_id_migration(&self.params, chapter_key)
	}
}
