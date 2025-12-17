#![no_std]
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap,
	Home, HomeComponent, HomeLayout, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, borrow::ToOwned, fmt::format, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		html::Element,
		net::{Request, TimeUnit, set_rate_limit},
	},
	prelude::*,
};

use crate::model::{ChapterResponse, ComixChapter, ComixManga, ComixResponse};

mod home;
mod model;
// use iken::{IKen, Impl, Params};

const BASE_URL: &str = "https://comix.com";
const API_URL: &str = "https://comix.to/api/v2";

struct Comix;

fn is_official_like(ch: &ComixChapter) -> bool {
	ch.scanlation_group_id == 9275 || ch.is_official == 1
}

fn is_better(new_ch: &ComixChapter, cur: &ComixChapter) -> bool {
	let official_new = is_official_like(new_ch);
	let official_cur = is_official_like(cur);

	if official_new && !official_cur {
		return true;
	}
	if !official_new && official_cur {
		return false;
	}

	if new_ch.votes > cur.votes {
		return true;
	}
	if new_ch.votes < cur.votes {
		return false;
	}

	new_ch.updated_at > cur.updated_at
}

fn dedup_insert(map: &mut HashMap<String, ComixChapter>, ch: ComixChapter) {
	let key = ch.number.to_string();
	match map.get(&key) {
		None => {
			map.insert(key, ch);
		}
		Some(current) => {
			if is_better(&ch, current) {
				map.insert(key, ch);
			}
		}
	}
}

impl Source for Comix {
	fn new() -> Self {
		set_rate_limit(5, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		// for filter in filters {
		// 	match filter {
		// 		FilterValue::Text { id, value } => todo!(),
		// 		FilterValue::Sort {
		// 			id,
		// 			index,
		// 			ascending,
		// 		} => todo!(),
		// 		FilterValue::Check { id, value } => todo!(),
		// 		FilterValue::Select { id, value } => todo!(),
		// 		FilterValue::MultiSelect {
		// 			id,
		// 			included,
		// 			excluded,
		// 		} => todo!(),
		// 		FilterValue::Range { id, from, to } => todo!(),
		// 	}
		// }
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
			let limit = 100;
			let mut page = 1;
			let mut chapter_map: HashMap<String, ComixChapter> = HashMap::new();
			loop {
				let url = format!(
					"{API_URL}/manga/{}/chapters?limit={}&page={}&order[number]=desc",
					manga.key, limit, page
				);

				let res = Request::get(url)?
					.send()?
					.get_json::<ComixResponse<ComixChapter>>()?;

				// insert/dedup this page's items
				for item in res.result.items {
					dedup_insert(&mut chapter_map, item);
				}

				// stop condition
				if res.result.pagination.current_page >= res.result.pagination.last_page {
					break;
				}

				page += 1;
			}

			// convert to aidoku::Chapter and set url field
			let mut chapters: Vec<Chapter> = chapter_map
				.into_values()
				.map(|item| {
					let url = Some(item.url(&manga));
					let mut ch: Chapter = item.into();
					ch.url = url;
					ch
				})
				.collect();

			// optional: keep deterministic ordering (desc by chapter number)
			chapters.sort_by(|a, b| {
				b.chapter_number
					.partial_cmp(&a.chapter_number)
					.unwrap_or(core::cmp::Ordering::Equal)
			});

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// let chapter_url = chapter.url.as_deref().unwrap_or("");
		let chapter_id = chapter.key;
		// Kotlin: val url = "${apiUrl}chapters/$chapterId"
		let url = format!("{API_URL}/chapters/{}", chapter_id);

		// Kotlin: GET(url, headers) then parse JSON
		let res = Request::get(url)?.send()?.get_json::<ChapterResponse>()?;

		let result = res
			.result
			.ok_or(error!("Chapter not found"))
			.unwrap_or_default();

		if result.images.is_empty() {
			return Ok(vec![]);
		}

		// Kotlin: result.images.mapIndexed { index, img -> Page(index, imageUrl = img.url) }
		let pages: Vec<Page> = result
			.images
			.into_iter()
			.enumerate()
			.map(|(index, img)| Page {
				// index: index as i32,
				content: PageContent::url(img.url),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

impl DeepLinkHandler for Comix {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		todo!()
	}
}

register_source!(Comix, Home, DeepLinkHandler);
