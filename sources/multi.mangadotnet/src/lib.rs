#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterKind, FilterValue,
	HashMap, Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, Result, Source,
	alloc::borrow::Cow,
	alloc::vec,
	alloc::{String, Vec},
	helpers::uri::QueryParameters,
	imports::std::send_partial_result,
	prelude::*,
};
use core::cmp::*;

mod helpers;
mod models;
mod settings;

use helpers::*;
use models::*;
use settings::*;

const BASE_URL: &str = "https://mangadot.net";

struct Mangadotnet;

impl Source for Mangadotnet {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut query_parameters = QueryParameters::new();

		if query.is_some() {
			query_parameters.push("search", query.as_deref());
		}

		query_parameters.push("page", Some(&format!("{page}")));

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

		if !hide_nsfw() {
			query_parameters.push("adult", Some("both"));
		}

		query_parameters.push("_routes", Some("pages/SearchPage"));

		let search_response: SearchPage =
			get_page_container_json_data(&format!("{BASE_URL}/search.data?{query_parameters}"))?;

		Ok(MangaPageResult {
			entries: search_response.results
				.map(|results| results.into_iter().map(Into::into).collect())
				.unwrap_or_default(),
			has_next_page: search_response.pagination
				.map(|p| p.current_page < p.total_pages)
				.unwrap_or_default(),
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let manga_detail_page: MangaDetailPage = get_page_container_json_data(&format!(
				"{BASE_URL}/manga/{}.data?_routes=pages/MangaDetailPage",
				manga.key
			))?;

			manga.copy_from(manga_detail_page.manga_data.manga.into());

			if needs_chapters {
				send_partial_result(&manga)
			}
		}

		if needs_chapters {
			let json: Vec<MangaChapter> =
				get_json_data(&format!("{BASE_URL}/api/manga/{}/chapters/list", manga.key))?;

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
					.unwrap_or(Ordering::Equal)
			});

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let json: MangaPage =
			get_json_data(&format!("{BASE_URL}/api/chapters/{}/images", chapter.key))?;

		Ok(json
			.images
			.into_iter()
			.map(|page_image| Page {
				content: PageContent::url(format!(
					"{BASE_URL}/{}",
					page_image.url.trim_start_matches('/')
				)),
				..Default::default()
			})
			.collect())
	}
}

const LATEST_UPDATES_LISTING_ID: &str = "latest_updates";
const RECENTLY_ADDED_LISTING_ID: &str = "recently_added";

impl ListingProvider for Mangadotnet {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let mut query_parameters = QueryParameters::new();

		if !hide_nsfw() {
			query_parameters.push("adult", Some("both"));
		}

		if page > 1 {
			query_parameters.push("page", Some(&format!("{}", page)));
		}

		match listing.id.as_str() {
			LATEST_UPDATES_LISTING_ID => {
				query_parameters.push("_routes", Some("pages/ViewAllPage"));

				let view_all_page: ViewAllPage = get_page_container_json_data(&format!(
					"{BASE_URL}/view-all/latest-updates.data?{}",
					query_parameters
				))?;

				Ok(MangaPageResult {
					entries: view_all_page
						.data
						.manga_list
						.into_iter()
						.map(Into::into)
						.collect(),
					has_next_page: view_all_page.data.pagination.current_page
						< view_all_page.data.pagination.total_pages,
				})
			}

			RECENTLY_ADDED_LISTING_ID => {
				query_parameters.push("_routes", Some("pages/ViewAllPage"));

				let view_all_page: ViewAllPage = get_page_container_json_data(&format!(
					"{BASE_URL}/view-all/recently-added.data?{}",
					query_parameters
				))?;

				Ok(MangaPageResult {
					entries: view_all_page
						.data
						.manga_list
						.into_iter()
						.map(Into::into)
						.collect(),
					has_next_page: view_all_page.data.pagination.current_page
						< view_all_page.data.pagination.total_pages,
				})
			}

			_ => bail!("Invalid listing id: {}", listing.id),
		}
	}
}

impl Home for Mangadotnet {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Latest Updates".into()),
					subtitle: Some("New Chapters".into()),
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Recently Added".into()),
					subtitle: Some("New Titles".into()),
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Most Tracked".into()),
					subtitle: Some("Reader Favorites".into()),
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Top Rated".into()),
					subtitle: Some("Highest Scores".into()),
					value: HomeComponentValue::empty_scroller(),
				},
			],
		}));

		let home_page_json: HomePage =
			get_page_container_json_data(&format!("{BASE_URL}/_root.data?_routes=pages/HomePage"))?;

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Latest Updates".into()),
					subtitle: Some("New Chapters".into()),
					value: HomeComponentValue::Scroller {
						entries: home_page_json
							.sections_data
							.sections
							.latest_updates
							.items
							.into_iter()
							.map(Into::into)
							.collect(),
						listing: Some(Listing {
							id: LATEST_UPDATES_LISTING_ID.into(),
							name: "Latest Updates".into(),
							..Default::default()
						}),
					},
				},
				HomeComponent {
					title: Some("Recently Added".into()),
					subtitle: Some("New Titles".into()),
					value: HomeComponentValue::Scroller {
						entries: home_page_json
							.sections_data
							.sections
							.recently_added
							.items
							.into_iter()
							.map(Into::into)
							.collect(),
						listing: Some(Listing {
							id: "recently_added".into(),
							name: "Recently Added".into(),
							..Default::default()
						}),
					},
				},
				HomeComponent {
					title: Some("Most Tracked".into()),
					subtitle: Some("Reader Favorites".into()),
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
					title: Some("Top Rated".into()),
					subtitle: Some("Highest Scores".into()),
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
		})
	}
}

impl DeepLinkHandler for Mangadotnet {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		// https://mangadot.net/manga/6953
		// https://mangadot.net/chapter/533518#p=1
		// https://mangadot.net/chapter/151856?source=user#p=1

		let mut segments = path.trim_start_matches('/').split('/');

		if let (Some(kind), Some(id)) = (segments.next(), segments.next()) {
			return Ok(match kind {
				"manga" => Some(DeepLinkResult::Manga { key: id.into() }),
				_ => None,
			});
		}

		Ok(None)
	}
}

impl DynamicFilters for Mangadotnet {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut query_parameters = QueryParameters::new();

		if !hide_nsfw() {
			query_parameters.push("adult", Some("both"));
		}

		query_parameters.push("_routes", Some("pages/SearchPage"));

		let search_response: SearchPage =
			get_page_container_json_data(&format!("{BASE_URL}/search.data?{query_parameters}"))?;

		Ok(vec![Filter {
			id: Cow::from("genre"),
			title: Some("Genres".into()),
			hide_from_header: None,
			kind: FilterKind::MultiSelect {
				is_genre: true,
				can_exclude: true,
				uses_tag_style: true,
				options: search_response
					.all_genres
					.into_iter()
					.map(Into::into)
					.collect(),
				ids: None,
				default_included: None,
				default_excluded: None,
			},
		}])
	}
}

register_source!(
	Mangadotnet,
	ListingProvider,
	Home,
	DeepLinkHandler,
	DynamicFilters
);
