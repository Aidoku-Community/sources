#![no_std]

mod html;
mod json;
mod net;

use aidoku::{
	alloc::{string::ToString as _, String, Vec},
	imports::net::Request,
	prelude::*,
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, Result, Source,
};
use html::MangaPage as _;
use net::Url;

pub const BASE_URL: &str = "https://m.happymh.com";

fn handle_cover_url(url: String) -> String {
	url
}

struct Happymh;

impl Source for Happymh {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<aidoku::FilterValue>,
	) -> Result<MangaPageResult> {
		let url = Url::from_query_or_filters(query.as_deref(), page, &filters)?;
		let json: serde_json::Value = url.request()?.send()?.get_json()?;
		let data = json
			.as_object()
			.ok_or_else(|| AidokuError::message("Expected JSON object"))?;
		let data = data
			.get("data")
			.and_then(|v| v.as_object())
			.ok_or_else(|| AidokuError::message("Expected data object"))?;
		let list = data
			.get("items")
			.and_then(|v| v.as_array())
			.ok_or_else(|| AidokuError::message("Expected items array"))?;
		let mut mangas: Vec<Manga> = Vec::new();

		for item in list {
			let item = match item.as_object() {
				Some(item) => item,
				None => continue,
			};
			let id = item
				.get("manga_code")
				.and_then(|v| v.as_str())
				.unwrap_or_default()
				.to_string();
			let cover = item
				.get("cover")
				.and_then(|v| v.as_str())
				.unwrap_or_default()
				.to_string();
			let title = item
				.get("name")
				.and_then(|v| v.as_str())
				.unwrap_or_default()
				.to_string();
			mangas.push(Manga {
				key: id,
				cover: Some(cover),
				title,
				..Default::default()
			});
		}

		Ok(MangaPageResult {
			entries: mangas,
			// There is no page information in the response, assuming it is always true
			// The API returns null results for non-existent page requests
			has_next_page: true,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let manga_page = Url::manga(manga.key.clone()).request()?.html()?;
			manga_page.update_details(&mut manga)?;
		}

		if needs_chapters {
			manga.chapters = Some(json::chapter_list::ChapterList::get_chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		json::page_list::PageList::get_pages(manga.key, chapter.key)
	}
}

impl ImageRequestProvider for Happymh {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Happymh {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let url = url.trim_start_matches(BASE_URL);
		let mut splits = url.split('/').skip(1);
		let deep_link_result = match splits.next() {
			Some("manga") => match (splits.next(), splits.next()) {
				(Some(manga_id), None) => Some(DeepLinkResult::Manga {
					key: manga_id.into(),
				}),
				(Some(manga_id), Some(chapter_id)) => Some(DeepLinkResult::Chapter {
					manga_key: manga_id.into(),
					key: chapter_id.into(),
				}),
				_ => None,
			},
			_ => None,
		};
		Ok(deep_link_result)
	}
}

impl ListingProvider for Happymh {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"day" => format!("{}/rank/day?page={}", BASE_URL, page),
			"week" => format!("{}/rank/week?page={}", BASE_URL, page),
			"month" => format!("{}/rank/month?page={}", BASE_URL, page),
			"dayBookcasesOne" => format!("{}/rank/dayBookcasesOne?page={}", BASE_URL, page),
			"weekBookcasesOne" => format!("{}/rank/weekBookcasesOne?page={}", BASE_URL, page),
			"monthBookcasesOne" => format!("{}/rank/monthBookcasesOne?page={}", BASE_URL, page),
			"voteNumMonthRank" => format!("{}/rank/voteNumMonthRank?page={}", BASE_URL, page),
			"voteRank" => format!("{}/rank/voteRank?page={}", BASE_URL, page),
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		let html = Request::get(url)?.header("Origin", BASE_URL).html()?;

		html.manga_page_result()
	}
}

register_source!(
	Happymh,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
