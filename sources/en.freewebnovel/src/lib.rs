#![no_std]
use aidoku::{
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, Listing,
	ListingProvider, Manga, MangaPageResult, Page, Result, Source,
	alloc::{String, Vec},
	prelude::*,
};

struct freewebnovel;

impl Source for freewebnovel {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		_query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		Err(AidokuError::Unimplemented)
	}

	fn get_manga_update(
		&self,
		_manga: Manga,
		_needs_details: bool,
		_needs_chapters: bool,
	) -> Result<Manga> {
		Err(AidokuError::Unimplemented)
	}

	fn get_page_list(&self, _manga: Manga, _chapter: Chapter) -> Result<Vec<Page>> {
		Err(AidokuError::Unimplemented)
	}
}

impl ListingProvider for freewebnovel {
	fn get_manga_list(&self, _listing: Listing, _page: i32) -> Result<MangaPageResult> {
		Err(AidokuError::Unimplemented)
	}
}

impl Home for freewebnovel {
	fn get_home(&self) -> Result<HomeLayout> {
		Err(AidokuError::Unimplemented)
	}
}

impl DeepLinkHandler for freewebnovel {
	fn handle_deep_link(&self, _url: String) -> Result<Option<DeepLinkResult>> {
		Err(AidokuError::Unimplemented)
	}
}

register_source!(freewebnovel, ListingProvider, Home, DeepLinkHandler);
