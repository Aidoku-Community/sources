#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout,
    ImageRequestProvider, ListingProvider, Manga, MangaPageResult,
    Page, PageContext, Result, Source, Viewer,
    alloc::{string::String, vec::Vec},
    imports::net::Request,
};
use core::cell::RefCell;

mod helpers;
mod imp;
mod models;

pub use helpers::strip_html;
pub use imp::Impl;
pub use models::*;

pub(crate) const PAGE_SIZE: usize = 20;
pub(crate) const SERIES_TTL: i64 = 300;

pub type SeriesCache = Option<(Vec<(String, AllSeriesItem)>, i64)>;

pub struct Params {
    pub base_url: &'static str,
    pub viewer: Viewer,
}

pub struct Guya<T: Impl> {
    inner: T,
    params: Params,
    cache: RefCell<SeriesCache>,
}

impl<T: Impl> Source for Guya<T> {
    fn new() -> Self {
        let inner = T::new();
        let params = inner.params();
        Self { inner, params, cache: RefCell::new(None) }
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let mut cache = self.cache.borrow_mut();
        self.inner.get_search_manga_list(&self.params, query, page, filters, &mut cache)
    }

    fn get_manga_update(
        &self,
        manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        self.inner.get_manga_update(&self.params, manga, needs_details, needs_chapters)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        self.inner.get_page_list(&self.params, manga, chapter)
    }
}

impl<T: Impl> ListingProvider for Guya<T> {
    fn get_manga_list(&self, listing: aidoku::Listing, page: i32) -> Result<MangaPageResult> {
        let mut cache = self.cache.borrow_mut();
        self.inner.get_manga_list(&self.params, listing, page, &mut cache)
    }
}

impl<T: Impl> Home for Guya<T> {
    fn get_home(&self) -> Result<HomeLayout> {
        let mut cache = self.cache.borrow_mut();
        self.inner.get_home(&self.params, &mut cache)
    }
}

impl<T: Impl> DeepLinkHandler for Guya<T> {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        self.inner.handle_deep_link(&self.params, url)
    }
}

impl<T: Impl> ImageRequestProvider for Guya<T> {
    fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
        self.inner.get_image_request(&self.params, url, context)
    }
}
