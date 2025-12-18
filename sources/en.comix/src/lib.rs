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

use crate::model::{ChapterResponse, ComixChapter, ComixManga, ComixResponse, ResultData};

mod home;
mod model;
// use iken::{IKen, Impl, Params};

const BASE_URL: &str = "https://comix.com";
const API_URL: &str = "https://comix.to/api/v2";

const INCLUDES: [&str; 6] = [
	"demographic",
	"genre",
	"theme",
	"author",
	"artist",
	"publisher",
];

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
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" => todo!(),
					_ => return Err(AidokuError::Message(("Invalid text filter id".into()))),
				},
				FilterValue::Sort {
					index, ascending, ..
				} => {
					let key = format!(
						"order[{}]",
						match index {
							0 => "relevance",
							1 => "views_30d",
							2 => "chapter_updated_at",
							3 => "created_at",
							4 => "title",
							5 => "year",
							6 => "views_total",
							7 => "follows_total",
							_ =>
								return Err(AidokuError::Message(
									"Invalid sort filter index".into()
								)),
						}
					);
					qs.push(&key, Some(if ascending { "asc" } else { "desc" }));
				}
				// FilterValue::Check { id, value } => todo!(),
				// FilterValue::Select { id, value } => todo!(),
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"status" => {
						for id in included {
							qs.push("statuses[]", Some(&id));
						}
					}
					"type" => {
						for id in included {
							qs.push("types[]", Some(&id));
						}
					}
					"genre" => {
						for id in included {
							qs.push("genres[]", Some(&id));
						}
						for id in excluded {
							qs.push("genres[]", Some(&format!("-{}", id)));
						}
					}
					"demographic" => {
						for id in included {
							qs.push("demographics[]", Some(&id));
						}
						for id in excluded {
							qs.push("demographics[]", Some(&format!("-{}", id)));
						}
					}
					_ => {
						return Err(AidokuError::Message(
							("Invalid multi-select filter id".into()),
						));
					}
				},
				// FilterValue::Range { id, from, to } => continue,
				_ => continue,
			}
		}

		if let Some(query) = query {
			qs.push("keyword", Some(&query));
			qs.remove_all("order[views_30d]");
			qs.set("order[relevance]", Some("desc".into()));
		}

		qs.push("limit", Some("50".into()));
		qs.push("page", Some(&page.to_string()));

		let url = format!("{API_URL}/manga?{qs}");
		println!("{}", url);
		let (entries, has_next_page) = Request::get(url)?
			.send()?
			.get_json::<ComixResponse<ResultData<ComixManga>>>()
			.map(|res| {
				(
					res.result
						.items
						.into_iter()
						.map(Into::into)
						.collect::<Vec<Manga>>(),
					res.result.pagination.current_page < res.result.pagination.last_page,
				)
			})?;
		// let has_next_page = results.pagination.current_page < results.pagination.last_page;

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
		// todo!()
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let base_url = format!("{API_URL}/manga");
		if needs_details {
			let mut qs = QueryParameters::new();
			for item in INCLUDES {
				qs.push("includes[]", Some(item));
			}
			let url = format!("{base_url}/{}?{qs}", manga.key);
			manga.copy_from(
				Request::get(url)?
					.send()?
					.get_json::<ComixResponse<ComixManga>>()?
					.result
					.into(),
			);
		}

		if needs_chapters {
			let limit = 100;
			let mut page = 1;
			let mut chapter_map: HashMap<String, ComixChapter> = HashMap::new();
			loop {
				let url = format!(
					"{base_url}/{}/chapters?limit={}&page={}&order[number]=desc",
					manga.key, limit, page
				);

				let res = Request::get(url)?
					.send()?
					.get_json::<ComixResponse<ResultData<ComixChapter>>>()?;

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
			.map(|(_index, img)| Page {
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
