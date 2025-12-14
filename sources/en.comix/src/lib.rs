#![no_std]
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeComponent, HomeLayout, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, borrow::ToOwned, fmt::format, string::ToString, vec},
	imports::{html::Element, net::Request, std::send_partial_result},
	prelude::*,
};

use crate::model::{ComixChapter, ComixManga, ComixResponse};

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
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {}

		if needs_chapters {
			let limit: i64 = 500;
			let url = format!(
				"{API_URL}/manga/{}/chapters?limit={}&page={}&ordern[number]=desc",
				limit, manga.key, 1
			);
			// .header("Referer", &format!("{}/", params.base_url))
			let res = Request::get(url)?
				.send()?
				.get_json::<ComixResponse<ComixChapter>>()?;

			let total = res.result.pagination.total;

			let chapters: Vec<Chapter> = res
				.result
				.items
				.into_iter()
				.map(|item| {
					let url = Some(item.url(&manga));
					let mut ch: Chapter = item.into();
					ch.url = url; // change 1 field (example)
					ch
				})
				.collect();

			let (mut chapters, total) = (chapters, total);
			let pages = (total + limit - 1) / limit;

			for page in 2..=pages {
				let url = format!(
					"{API_URL}/manga/{}/chapters?limit={limit}&page={page}&ordern[number]=desc",
					manga.key
				);

				let res = Request::get(url)?
					.send()?
					.get_json::<ComixResponse<ComixChapter>>()?;

				chapters.extend(res.result.items.into_iter().map(|item| {
					let url = Some(item.url(&manga));
					let mut ch: Chapter = item.into();
					ch.url = url; // change 1 field (example)
					ch
				}));
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		todo!()
	}
}

impl Home for Comix {
	fn get_home(&self) -> Result<HomeLayout> {
		let url = format!("{API_URL}/manga?order[views_30d]=desc&limit=28");

		let mut manga_request = Request::get(&url)?.send()?;
		let manga_response = manga_request.get_json::<ComixResponse<ComixManga>>()?;
		let manga_list = manga_response
			.result
			.items
			.into_iter()
			.map(|item| Manga {
				key: item.hash_id.to_string(),
				title: item.title,
				cover: Some(item.poster.medium.to_string()),
				..Default::default()
			})
			.collect();

		Ok(HomeLayout {
			components: vec![HomeComponent {
				title: Some("Hot Updates".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: manga_list,
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
