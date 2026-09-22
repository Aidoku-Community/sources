#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicListings, FilterValue, ImageRequestProvider,
	Listing, ListingProvider, Manga, MangaPageResult, Page, PageContext, Source,
	alloc::{String, Vec, vec},
	imports::error::Result,
	imports::net::Request,
	register_source,
};

mod helper;
mod parser;

pub struct Webtoon;

impl Source for Webtoon {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		parser::parse_search_manga_list(query, page, filters)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		parser::parse_manga_update(manga, needs_details, needs_chapters)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		parser::parse_page_list(&manga.key, &chapter.key)
	}
}

impl ListingProvider for Webtoon {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		parser::parse_manga_listing(listing, page)
	}
}

impl DynamicListings for Webtoon {
	fn get_dynamic_listings(&self) -> Result<Vec<Listing>> {
		if !helper::get_canvas_series() {
			return Ok(Vec::new());
		}
		Ok(vec![
			Listing {
				id: String::from("canvas_latest"),
				name: String::from("Canvas Latest"),
				..Default::default()
			},
			Listing {
				id: String::from("canvas_popular"),
				name: String::from("Canvas Popular"),
				..Default::default()
			},
			Listing {
				id: String::from("canvas_top"),
				name: String::from("Canvas Top"),
				..Default::default()
			},
		])
	}
}

impl ImageRequestProvider for Webtoon {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		parser::get_image_request(url)
	}
}

impl DeepLinkHandler for Webtoon {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		parser::parse_deep_link(&url)
	}
}

register_source!(
	Webtoon,
	ListingProvider,
	DynamicListings,
	ImageRequestProvider,
	DeepLinkHandler
);
