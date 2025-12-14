#![no_std]
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeComponent, HomeLayout, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, borrow::ToOwned, fmt::format, string::ToString, vec},
	imports::{html::Element, net::Request, std::send_partial_result},
	prelude::*,
};

use crate::model::ApiResponse;

mod model;
// use iken::{IKen, Impl, Params};

const BASE_URL: &str = "https://comix.com";
const API_URL: &str = "https://comix.to/api/v2";

struct Comix;

impl Source for Comix {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		todo!()
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		todo!()
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		todo!()
	}
}

impl Home for Comix {
	fn get_home(&self) -> Result<HomeLayout> {
		let url = format!("{API_URL}/manga?order[views_30d]=desc&limit=28");

		let mangaResponse = Request::get(&url)?.send()?.get_json::<ApiResponse>()?;
		let mangaList = mangaResponse
			.result
			.items
			.into_iter()
			.map(|item| Manga {
				key: item.manga_id.to_string(),
				title: item.title,
				cover: item.poster.medium.into(),
				..Default::default()
			})
			.collect();

		Ok(HomeLayout {
			components: vec![HomeComponent {
				title: Some("Hot Updates".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: mangaList,
					auto_scroll_interval: None,
				},
			}],
		})
	}
}

impl DeepLinkHandler for Comix {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		todo!()
	}
}

register_source!(Comix, Home, DeepLinkHandler);
