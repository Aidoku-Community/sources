#![no_std]
use aidoku::{
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, Listing,
	ListingKind, Manga, MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::net::{Request, TimeUnit, set_rate_limit},
	prelude::*,
};

use crate::model::{ChapterResponse, ComixChapter, ComixManga, ComixResponse, ResultData};

mod home;
mod model;
mod settings;

const BASE_URL: &str = "https://comix.com";
const API_URL: &str = "https://comix.to/api/v2";

const NSFW_GENRE_IDS: [&str; 6] = ["87264", "8", "87265", "13", "87266", "87268"];
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
				_ => continue,
			}
		}

		if let Some(query) = query {
			qs.push("keyword", Some(&query));
			qs.remove_all("order[views_30d]");
			qs.set("order[relevance]", Some("desc".into()));
		}

		if settings::get_nsfw() {
			for item in NSFW_GENRE_IDS {
				qs.push("gernes[]", Some(&format!("-{item}")));
			}
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

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
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
			let deduplicate = settings::get_dedupchapter();
			let mut chapter_map: HashMap<String, ComixChapter> = HashMap::new();
			let mut chapter_list: Vec<ComixChapter> = Vec::new();
			loop {
				let url = format!(
					"{base_url}/{}/chapters?limit={}&page={}&order[number]=desc",
					manga.key, limit, page
				);

				let res = Request::get(url)?
					.send()?
					.get_json::<ComixResponse<ResultData<ComixChapter>>>()?;

				let items = res.result.items;

				if deduplicate {
					for item in items {
						dedup_insert(&mut chapter_map, item);
					}
				} else {
					chapter_list.extend(items);
				}

				if res.result.pagination.current_page >= res.result.pagination.last_page {
					break;
				}

				page += 1;
			}

			let raw_chapters = if deduplicate {
				chapter_map.into_values().collect::<Vec<_>>()
			} else {
				chapter_list
			};

			let mut chapters: Vec<Chapter> = raw_chapters
				.into_iter()
				.map(|item| {
					let url = Some(item.url(&manga));
					let mut ch: Chapter = item.into();
					ch.url = url;
					ch
				})
				.collect();

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
		let chapter_id = chapter.key;
		let url = format!("{API_URL}/chapters/{}", chapter_id);

		let res = Request::get(url)?.send()?.get_json::<ChapterResponse>()?;

		let result = res
			.result
			.ok_or(error!("Chapter not found"))
			.unwrap_or_default();

		if result.images.is_empty() {
			return Ok(vec![]);
		}

		let pages: Vec<Page> = result
			.images
			.into_iter()
			.enumerate()
			.map(|(_index, img)| Page {
				content: PageContent::url(img.url),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

impl DeepLinkHandler for Comix {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}

		let key = &url[BASE_URL.len()..];
		const LATEST_QUERY: &str = "order[chapter_updated_at]";
		let sections: Vec<&str> = key.split('/').filter(|s| !s.is_empty()).collect();
		if key.contains(LATEST_QUERY) {
			Ok(Some(DeepLinkResult::Listing(Listing {
				id: "latest".to_string(),
				name: "Latest Releases".to_string(),
				kind: ListingKind::Default,
			})))
		} else if sections.len() == 2 && sections[0] == "title" {
			// ex: https://comix.to/title/rm7l-after-becoming-financially-free-they-offered-their-loyalty
			let full_slug = sections[1];

			if let Some(id) = full_slug.split('-').next() {
				return Ok(Some(DeepLinkResult::Manga {
					key: id.to_string(),
				}));
			} else {
				Ok(None)
			}
		} else if sections.len() == 3 && sections[0] == "title" {
			// ex: https://comix.to/title/rm7l-after-becoming.../7206380-chapter-63

			let manga_id = sections[1].split('-').next();
			let chapter_id = sections[2].split('-').next();

			if let (Some(m_id), Some(c_id)) = (manga_id, chapter_id) {
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: m_id.to_string(),
					key: c_id.to_string(),
				}))
			} else {
				Ok(None)
			}
		} else {
			Ok(None)
		}
	}
}

register_source!(Comix, Home, ListingProvider, DeepLinkHandler);
