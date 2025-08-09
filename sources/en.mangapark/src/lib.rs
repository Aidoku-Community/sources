#![no_std]
use aidoku::{
	alloc::{String, Vec}, imports::net::Request, prelude::*, AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, Page, Result, Source
};

mod filter;

const BASE_URL: &str = "https://mangapark.com";

const PAGE_SIZE: i32 = 16;

struct MangaPark;

impl Source for MangaPark {
	fn new() -> Self {
		Self
	}

	/* Quite challenging to select the tag with all the articles
	
	 */
	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		//Ex: https://mangapark.com/title/346393-en-an-introvert-s-hookup-hiccups-this-gyaru-is-head-over-heels-for-me
		// what is the page component suppose to be?
		let url = format!("{}/title/{}-", BASE_URL,page,);

		let offset = (page-1) * PAGE_SIZE;

		//TODO: set entries, has_net_page
		// Ok(MangaPageResult{
		// 	entries,
		// 	has_next_page
		// });
	}

	fn get_manga_update(
		&self,
		mut manga: Manga, //From weebcentral
		// _manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{BASE_URL}{}", manga.key);
		println!(manga_url);
		if needs_details{
			let html = Request::get(&manga_url)?.html()?;
		}

		if needs_chapters{

		}
		// Err(AidokuError::Unimplemented)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Err(AidokuError::Unimplemented)
	}
}

// Listing for /latest
impl ListingProvider for MangaPark {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		Err(AidokuError::Unimplemented)
	}
}

impl Home for MangaPark {
	fn get_home(&self) -> Result<HomeLayout> {
		Err(AidokuError::Unimplemented)
	}
}

impl DeepLinkHandler for MangaPark {
	fn handle_deep_link(&self, _url: String) -> Result<Option<DeepLinkResult>> {
		Err(AidokuError::Unimplemented)
	}
}

register_source!(MangaPark, ListingProvider, Home, DeepLinkHandler);
