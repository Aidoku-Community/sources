#![no_std]
use crate::helpers::{dedup_insert, resolve_ptr_table_json, to_json_data};
use crate::models::{
	HomePageResponse, MangaChapter, MangaDetailResponse, MangaPage, SearchResponse,
};
use crate::settings::deduped_chapter;
use aidoku::{
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, Home,
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Listing, ListingProvider,
	Manga, MangaPageResult, Page, PageContent, Result, Source,
	alloc::string::ToString,
	alloc::vec,
	alloc::{String, Vec},
	helpers::uri::QueryParameters,
	imports::net::Request,
	imports::std::send_partial_result,
	prelude::*,
};
use core::cmp::Ordering::Equal;
use serde_json::Value;

mod helpers;
mod models;
mod settings;

const BASE_URL: &str = "https://mangadot.net";

struct Mangadotnet;

impl Source for Mangadotnet {
	fn new() -> Self {
		// This is just a script to generate static genres list and output into aidoku logcat.
		// Please do not use this in production xD (No Python script as this site had an aggressive
		// CF that makes it impossible to do so)
		/*
		if let Ok(response) = Request::get("https://mangadot.net/search.data?_routes=pages/SearchPage") {
			if let Ok(json) = response.json_owned::<Vec<Value>>() {
				if let Ok(search_response_ptr_table) = resolve_ptr_table_json(&json, 0) {
					if let Ok(search_response) = to_json_data::<SearchResponse>(search_response_ptr_table) {
						println!("{:?}", search_response.all_genres)
					}
				}
			}
		}
		*/
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut query_parameters = QueryParameters::new();

		if let Some(query) = query {
			query_parameters.push("search", Some(&query));
		}

		query_parameters.push("page", Some(&page.to_string()));

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => {
					query_parameters.push(&id, Some(&value));
				}

				FilterValue::Sort {
					index, ascending, ..
				} => {
					let value = match index {
						0 => "relevance",
						1 => "latest",
						2 => "alphabetical",
						3 => "chapters",
						4 => "views",
						5 => "tracked",
						6 => "rating",
						_ => bail!("Invalid sort index"),
					};
					let order = match ascending {
						true => "asc",
						false => "desc",
					};
					query_parameters.push("sortBy", Some(value));
					query_parameters.push("sortOrder", Some(order));
				}

				FilterValue::Select { id, value } => {
					query_parameters.push(&id, Some(&value));
				}

				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					for include_id in included {
						query_parameters.push(&id, Some(&include_id));
					}

					for excluded_id in excluded {
						let id = format!("-{excluded_id}");
						query_parameters.push(&id, Some(&id));
					}
				}

				_ => bail!("Invalid filter"),
			}
		}

		if !settings::hide_nsfw() {
			query_parameters.push("adult", Some("1"));
		}

		query_parameters.push("_routes", Some("pages/SearchPage"));

		let json: Vec<Value> =
			Request::get(format!("{BASE_URL}/search.data?{query_parameters}"))?.json_owned()?;
		let search_response_ptr_table = resolve_ptr_table_json(&json, 0)?;
		let search_response: SearchResponse = to_json_data(search_response_ptr_table)?;

		Ok(MangaPageResult {
			entries: if let Some(results) = search_response.results {
				results.into_iter().map(Into::into).collect()
			} else {
				Vec::new()
			},
			has_next_page: if let Some(pagination) = search_response.pagination {
				pagination.current_page < pagination.total_pages
			} else {
				false
			},
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let json: Vec<Value> = Request::get(format!(
				"{BASE_URL}/manga/{}.data?_routes=pages/MangaDetailPage",
				manga.key
			))?
			.json_owned()?;
			let manga_detail_response_ptr_table = resolve_ptr_table_json(&json, 0)?;
			let manga_detail_response: MangaDetailResponse =
				to_json_data(manga_detail_response_ptr_table)?;

			manga.copy_from(manga_detail_response.manga_data.manga.into());

			if needs_chapters {
				send_partial_result(&manga)
			}
		}

		if needs_chapters {
			let json: Vec<MangaChapter> =
				Request::get(format!("{BASE_URL}/api/manga/{}/chapters/list", manga.key))?
					.json_owned()?;

			let mut chapter_map: HashMap<String, MangaChapter> = HashMap::new();
			let mut chapter_list: Vec<MangaChapter> = Vec::new();

			if deduped_chapter() {
				for manga in json {
					dedup_insert(&mut chapter_map, manga);
				}
			} else {
				chapter_list.extend(json);
			}

			let mut chapters: Vec<Chapter> = if deduped_chapter() {
				chapter_map.into_values().map(Into::into).collect()
			} else {
				chapter_list.into_iter().map(Into::into).collect()
			};

			chapters.sort_by(|a, b| {
				b.chapter_number
					.partial_cmp(&a.chapter_number)
					.unwrap_or(Equal)
			});

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let json: MangaPage =
			Request::get(format!("{BASE_URL}/api/chapters/{}/images", chapter.key))?
				.json_owned()?;

		Ok(json
			.images
			.into_iter()
			.map(|page_image| Page {
				content: PageContent::url(format!(
					"{BASE_URL}/{}",
					page_image.url.strip_prefix("/").unwrap_or(&page_image.url)
				)),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for Mangadotnet {
	fn get_manga_list(&self, _listing: Listing, _page: i32) -> Result<MangaPageResult> {
		Err(AidokuError::Unimplemented)
	}
}

impl Home for Mangadotnet {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Latest Updates".to_string()),
					subtitle: Some("New Chapters".to_string()),
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Recently Added".to_string()),
					subtitle: Some("New Titles".to_string()),
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Most Tracked".to_string()),
					subtitle: Some("Reader Favorites".to_string()),
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Top Rated".to_string()),
					subtitle: Some("Highest Scores".to_string()),
					value: HomeComponentValue::empty_scroller(),
				},
			],
		}));

		let json: Vec<Value> =
			Request::get(format!("{BASE_URL}/_root.data?_routes=pages/HomePage"))?.json_owned()?;
		let home_page_ptr_table_json = resolve_ptr_table_json(&json, 0)?;
		let home_page_json: HomePageResponse = to_json_data(home_page_ptr_table_json)?;

		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Latest Updates".to_string()),
					subtitle: Some("New Chapters".to_string()),
					value: HomeComponentValue::Scroller {
						entries: home_page_json
							.sections_data
							.sections
							.latest_updates
							.items
							.into_iter()
							.map(Into::into)
							.collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Recently Added".to_string()),
					subtitle: Some("New Titles".to_string()),
					value: HomeComponentValue::Scroller {
						entries: home_page_json
							.sections_data
							.sections
							.recently_added
							.items
							.into_iter()
							.map(Into::into)
							.collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Most Tracked".to_string()),
					subtitle: Some("Reader Favorites".to_string()),
					value: HomeComponentValue::Scroller {
						entries: home_page_json
							.sections_data
							.sections
							.most_tracked
							.items
							.into_iter()
							.map(Into::into)
							.collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Top Rated".to_string()),
					subtitle: Some("Highest Scores".to_string()),
					value: HomeComponentValue::Scroller {
						entries: home_page_json
							.sections_data
							.sections
							.top_rated
							.items
							.into_iter()
							.map(Into::into)
							.collect(),
						listing: None,
					},
				},
			],
		}));

		Ok(HomeLayout::default())
	}
}

impl DeepLinkHandler for Mangadotnet {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(&format!("{BASE_URL}/")) else {
			return Ok(None);
		};

		// https://mangadot.net/manga/6953
		// https://mangadot.net/chapter/533518#p=1
		// https://mangadot.net/chapter/151856?source=user#p=1

		let mut segments = path.split('/');

		if let (Some(kind), Some(id)) = (segments.next(), segments.next()) {
			return Ok(match kind {
				"manga" => Some(DeepLinkResult::Manga {
					key: id.to_string(),
				}),
				_ => None,
			});
		}

		Ok(None)
	}
}

register_source!(Mangadotnet, ListingProvider, Home, DeepLinkHandler);
