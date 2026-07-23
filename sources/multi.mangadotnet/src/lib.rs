#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicListings, FilterValue, HashMap, Home,
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Listing, ListingProvider,
	Manga, MangaPageResult, NotificationHandler, Page, PageContent, Result, Source,
	WebLoginHandler,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::std::send_partial_result,
	prelude::*,
};
use core::{cmp::*, ops::Deref};

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
			entries: search_response
				.results
				.map(|results| results.into_iter().map(Into::into).collect())
				.unwrap_or_default(),
			has_next_page: search_response
				.pagination
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

			if show_standalone_volume() {
				let volumes_json: Vec<MangaVolume> =
					get_json_data(&format!("{BASE_URL}/api/manga/{}/volumes", manga.key))?;

				let mut volumes: Vec<Chapter> = volumes_json.into_iter().map(Into::into).collect();
				chapters.append(&mut volumes);
			}

			chapters.sort_by(|a, b| {
				// 1) volume descending (None goes last)
				match b
					.volume_number
					.partial_cmp(&a.volume_number)
					.unwrap_or(Ordering::Equal)
				{
					Ordering::Equal => {
						match (&a.chapter_number, &b.chapter_number) {
							// 2) both have chapter numbers -> chapter descending
							(Some(a_ch), Some(b_ch)) => {
								b_ch.partial_cmp(a_ch).unwrap_or(Ordering::Equal)
							}

							// 3) chapter entries come before volume-only entries
							(Some(_), None) => Ordering::Less,
							(None, Some(_)) => Ordering::Greater,

							// 4) both volume-only
							(None, None) => Ordering::Equal,
						}
					}
					ord => ord,
				}
			});

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let json: MangaPage = if chapter.url.is_some_and(|url| url.contains("?source=user")) {
			get_json_data(&format!("{BASE_URL}/api/uploads/{}/images", chapter.key))?
		} else {
			get_json_data(&format!("{BASE_URL}/api/chapters/{}/images", chapter.key))?
		};

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
const MOST_TRACKED_LISTING_ID: &str = "most_tracked";
const TOP_RATED_LISTING_ID: &str = "top_rated";
const FOR_YOU_LISTING_ID: &str = "for_you";
const BOOKMARKS_LISTING_ID: &str = "bookmarks";

impl ListingProvider for Mangadotnet {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			BOOKMARKS_LISTING_ID => {
				let mut query_parameters = QueryParameters::new();
				query_parameters.push("_routes", Some("pages/BookmarksPage"));

				if page > 1 {
					query_parameters.push("page", Some(&format!("{page}")));
				}

				let bookmark_page: BookmarkPage = get_page_container_json_data(&format!(
					"{BASE_URL}/bookmark.data?{query_parameters}"
				))?;

				let mut manga_query_urls: Vec<String> =
					Vec::with_capacity(bookmark_page.data.entries.len());

				bookmark_page.data.entries.iter().for_each(|entry| {
					manga_query_urls.push(format!(
						"{BASE_URL}/manga/{}.data?_routes=pages/MangaDetailPage",
						entry.manga_id
					))
				});

				let manga_detail_pages: Vec<MangaDetailPage> =
					get_bulk_page_container_json_data(manga_query_urls.as_ref())?;

				Ok(MangaPageResult {
					entries: manga_detail_pages
						.into_iter()
						.map(|page| page.manga_data.manga.into())
						.collect(),
					has_next_page: bookmark_page.data.page * bookmark_page.data.per_page
						< bookmark_page.data.total,
				})
			}

			FOR_YOU_LISTING_ID => {
				let mut query_parameters = QueryParameters::new();

				if !hide_nsfw() {
					query_parameters.push("adult", Some("both"));
				}

				query_parameters.push("limit", Some("100"));

				let listing_data: ListingSectionData =
					get_json_data(&format!("{BASE_URL}/api/manga/for-you?{query_parameters}"))?;

				Ok(MangaPageResult {
					entries: listing_data.items.into_iter().map(Into::into).collect(),
					has_next_page: false,
				})
			}

			_ => {
				let mut query_parameters = QueryParameters::new();

				if !hide_nsfw() {
					query_parameters.push("adult", Some("both"));
				}

				if page > 1 {
					query_parameters.push("page", Some(&format!("{page}")));
				}

				if let Some(content_types) = get_default_content_types() {
					for content_type in content_types.split(",") {
						query_parameters.push("origin", Some(content_type));
					}
				}

				query_parameters.push("_routes", Some("pages/ViewAllPage"));

				let view_all_page: ViewAllPage = get_page_container_json_data(&format!(
					"{BASE_URL}/view-all/{}.data?{}",
					match listing.id.as_str() {
						LATEST_UPDATES_LISTING_ID => "latest-updates",
						RECENTLY_ADDED_LISTING_ID => "recently-added",
						MOST_TRACKED_LISTING_ID => "most-tracked",
						TOP_RATED_LISTING_ID => "top-rated",
						_ => bail!("Invalid listing id: {}", listing.id),
					},
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
		}
	}
}

impl Home for Mangadotnet {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut home_components = vec![
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
		];

		let is_logged_in = is_logged_in();

		if is_logged_in {
			home_components.push(HomeComponent {
				title: Some("For You".into()),
				subtitle: Some("Based on your bookmarks".into()),
				value: HomeComponentValue::empty_scroller(),
			});
		}

		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: home_components,
		}));

		for id in [
			"latest_updates",
			"recently_added",
			"most_tracked",
			"top_rated",
		] {
			let mut query_parameters = QueryParameters::new();
			query_parameters.push("id", Some(id));

			if !hide_nsfw() {
				query_parameters.push("adult", Some("both"));
			} else {
				query_parameters.push("adult", Some("0"));
			}

			if let Some(content_types) = get_default_content_types() {
				query_parameters.push("origin", Some(content_types.deref()));
			}

			query_parameters.push("limit", Some("21"));

			let listing_data: ListingSectionData =
				get_json_data(&format!("{BASE_URL}/api/manga/section?{query_parameters}"))?;

			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: match id {
					"latest_updates" => Some("Latest Updates".into()),
					"recently_added" => Some("Recently Added".into()),
					"most_tracked" => Some("Most Tracked".into()),
					"top_rated" => Some("Top Rated".into()),
					_ => None,
				},
				subtitle: match id {
					"latest_updates" => Some("New Chapters".into()),
					"recently_added" => Some("New Titles".into()),
					"most_tracked" => Some("Reader Favorites".into()),
					"top_rated" => Some("Highest Scores".into()),
					_ => None,
				},
				value: HomeComponentValue::Scroller {
					entries: listing_data.items.into_iter().map(Into::into).collect(),
					listing: match id {
						"latest_updates" => Some(Listing {
							id: LATEST_UPDATES_LISTING_ID.into(),
							name: "Latest Updates".into(),
							..Default::default()
						}),
						"recently_added" => Some(Listing {
							id: RECENTLY_ADDED_LISTING_ID.into(),
							name: "Recently Added".into(),
							..Default::default()
						}),
						"most_tracked" => Some(Listing {
							id: MOST_TRACKED_LISTING_ID.into(),
							name: "Most Tracked".into(),
							..Default::default()
						}),
						"top_rated" => Some(Listing {
							id: TOP_RATED_LISTING_ID.into(),
							name: "Top Rated".into(),
							..Default::default()
						}),
						_ => None,
					},
				},
			}));
		}

		if is_logged_in {
			let listing_data: ListingSectionData = get_json_data(&format!(
				"{BASE_URL}/api/manga/for-you?adult={}&limit=21",
				if hide_nsfw() { "0" } else { "both" }
			))?;

			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some("For You".into()),
				subtitle: Some("Based on your bookmarks".into()),
				value: HomeComponentValue::Scroller {
					entries: listing_data.items.into_iter().map(Into::into).collect(),
					listing: Some(Listing {
						id: FOR_YOU_LISTING_ID.into(),
						name: "For You".into(),
						..Default::default()
					}),
				},
			}));
		}

		Ok(HomeLayout::default())
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

				"chapter" => {
					if id.contains("source=user") {
						// This is a user uploaded chapter
						if let Some(chapter_id) = id.find('?').and_then(|idx| id.get(..idx)) {
							let json: MangaPage = get_json_data(&format!(
								"{BASE_URL}/api/uploads/{chapter_id}/images"
							))?;
							return Ok(Some(DeepLinkResult::Chapter {
								manga_key: json.manga.id.to_string(),
								key: json.chapter.id.to_string(),
							}));
						}
					} else {
						if let Some(chapter_id) = id.find('#').and_then(|idx| id.get(..idx)) {
							let json: MangaPage = get_json_data(&format!(
								"{BASE_URL}/api/chapters/{chapter_id}/images"
							))?;
							return Ok(Some(DeepLinkResult::Chapter {
								manga_key: json.manga.id.to_string(),
								key: json.chapter.id.to_string(),
							}));
						}
					}
					None
				}

				_ => None,
			});
		}

		Ok(None)
	}
}

impl DynamicListings for Mangadotnet {
	fn get_dynamic_listings(&self) -> Result<Vec<Listing>> {
		let mut listings: Vec<Listing> = Vec::new();

		if is_logged_in() {
			listings.push(Listing {
				id: BOOKMARKS_LISTING_ID.into(),
				name: "Your Bookmarks".into(),
				..Default::default()
			});
		}

		Ok(listings)
	}
}

impl NotificationHandler for Mangadotnet {
	fn handle_notification(&self, notification: String) {
		if notification == NOTIFICATION_RESET_KEY {
			reset_filters();
		}
	}
}

const LOGIN_COOKIE_KEY: &str = "ory_kratos_session";

impl WebLoginHandler for Mangadotnet {
	fn handle_web_login(&self, _key: String, cookies: HashMap<String, String>) -> Result<bool> {
		Ok(cookies.contains_key(LOGIN_COOKIE_KEY))
	}
}

register_source!(
	Mangadotnet,
	ListingProvider,
	Home,
	DeepLinkHandler,
	DynamicListings,
	NotificationHandler,
	WebLoginHandler
);
