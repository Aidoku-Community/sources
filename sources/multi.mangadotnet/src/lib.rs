#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterKind, FilterValue,
	HashMap, Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, ImageResponse,
	Listing, ListingProvider, Manga, MangaPageResult, NotificationHandler, Page, PageContent,
	PageContext, PageImageProcessor, Result, Source,
	alloc::{String, Vec, borrow::Cow, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::canvas::ImageRef,
	imports::net::Request,
	imports::std::send_partial_result,
	prelude::*,
};
use base64::Engine;
use core::{cell::RefCell, cmp::*, ops::Deref};
use serde::de::DeserializeOwned;
use serde_json::Value;

mod helpers;
mod models;
mod settings;
mod web;

use helpers::*;
use models::*;
use settings::*;
use web::*;

const BASE_URL: &str = "https://mangadot.net";
const CF_CHALLENGE_ERROR_MESSAGE: &str = "Response returned CF challenge page instead of JSON data. If problem persist, please clear the source cache and restart the application to resolve this issue.";

struct Mangadotnet {
	web_view: RefCell<MangaDotnetWebView>,
}

impl Mangadotnet {
	fn get_page_container_json_data<T>(&self, url: &str) -> Result<T>
	where
		T: DeserializeOwned,
	{
		if use_view_web_worker() {
			let json = self.web_view.borrow_mut().fetch(url, 0)?;
			let ptr_table_json = serde_json::from_str::<Vec<Value>>(&json)?;
			// Example: [{"_1":2},"pages/SearchPage",{"_3":4},"data",...]
			let json = resolve_ptr_table_json(&ptr_table_json, 0)?;
			// println!("{}", serde_json::to_string(&json)?);
			// Example: {"pages/SearchPage":{"data":{...}}}
			let Ok(page_container_json) =
				serde_json::from_value::<HashMap<String, PageContainer<T>>>(json)
			else {
				bail!("Invalid JSON data. Expected an object with page container data.")
			};
			let Some(page_container) = page_container_json.into_values().next() else {
				bail!("Page container data does not exists.")
			};
			Ok(page_container.data)
		} else {
			let response = Request::get(url)?.send()?;
			if response.status_code() == 403
				&& response
					.get_header("cf-mitigated")
					.is_some_and(|value| value == "challenge")
			{
				bail!("{CF_CHALLENGE_ERROR_MESSAGE}")
			} else {
				let ptr_table_json = response.get_json_owned::<Vec<Value>>()?;
				// Example: [{"_1":2},"pages/SearchPage",{"_3":4},"data",...]
				let json = resolve_ptr_table_json(&ptr_table_json, 0)?;
				// println!("{}", serde_json::to_string(&json)?);
				// Example: {"pages/SearchPage":{"data":{...}}}
				let Ok(page_container_json) =
					serde_json::from_value::<HashMap<String, PageContainer<T>>>(json)
				else {
					bail!("Invalid JSON data. Expected an object with page container data.")
				};
				let Some(page_container) = page_container_json.into_values().next() else {
					bail!("Page container data does not exists.")
				};
				Ok(page_container.data)
			}
		}
	}

	fn get_json_data<T>(&self, url: &str) -> Result<T>
	where
		T: DeserializeOwned,
	{
		if use_view_web_worker() {
			let json = self.web_view.borrow_mut().fetch(url, 0)?;
			let result = serde_json::from_str::<T>(&json)?;
			Ok(result)
		} else {
			let response = Request::get(url)?.send()?;
			if response.status_code() == 403
				&& response
					.get_header("cf-mitigated")
					.is_some_and(|value| value == "challenge")
			{
				bail!("{CF_CHALLENGE_ERROR_MESSAGE}")
			} else {
				response.get_json_owned::<T>()
			}
		}
	}
}

impl Source for Mangadotnet {
	fn new() -> Self {
		Self {
			web_view: RefCell::new(MangaDotnetWebView::new()),
		}
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

		let search_response: SearchPage = self
			.get_page_container_json_data(&format!("{BASE_URL}/search.data?{query_parameters}"))?;

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
			let manga_detail_page: MangaDetailPage =
				self.get_page_container_json_data(&format!(
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
				self.get_json_data(&format!("{BASE_URL}/api/manga/{}/chapters/list", manga.key))?;

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
		let json: MangaPage = if chapter.url.is_some_and(|url| url.contains("?source=user")) {
			self.get_json_data(&format!("{BASE_URL}/api/uploads/{}/images", chapter.key))?
		} else {
			self.get_json_data(&format!("{BASE_URL}/api/chapters/{}/images", chapter.key))?
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

impl ListingProvider for Mangadotnet {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let mut query_parameters = QueryParameters::new();

		if !hide_nsfw() {
			query_parameters.push("adult", Some("both"));
		}

		if page > 1 {
			query_parameters.push("page", Some(&format!("{}", page)));
		}

		if let Some(content_types) = get_default_content_types() {
			for content_type in content_types.split(",") {
				query_parameters.push("origin", Some(content_type));
			}
		}

		match listing.id.as_str() {
			LATEST_UPDATES_LISTING_ID => {
				query_parameters.push("_routes", Some("pages/ViewAllPage"));

				let view_all_page: ViewAllPage = self.get_page_container_json_data(&format!(
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

				let view_all_page: ViewAllPage = self.get_page_container_json_data(&format!(
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

			MOST_TRACKED_LISTING_ID => {
				query_parameters.push("_routes", Some("pages/ViewAllPage"));

				let view_all_page: ViewAllPage = self.get_page_container_json_data(&format!(
					"{BASE_URL}/view-all/most-tracked.data?{}",
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

			TOP_RATED_LISTING_ID => {
				query_parameters.push("_routes", Some("pages/ViewAllPage"));

				let view_all_page: ViewAllPage = self.get_page_container_json_data(&format!(
					"{BASE_URL}/view-all/top-rated.data?{}",
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
				self.get_json_data(&format!("{BASE_URL}/api/manga/section?{query_parameters}"))?;

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
							let json: MangaPage = self.get_json_data(&format!(
								"{BASE_URL}/api/uploads/{chapter_id}/images"
							))?;
							return Ok(Some(DeepLinkResult::Chapter {
								manga_key: json.manga.id.to_string(),
								key: json.chapter.id.to_string(),
							}));
						}
					} else {
						if let Some(chapter_id) = id.find('#').and_then(|idx| id.get(..idx)) {
							let json: MangaPage = self.get_json_data(&format!(
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

impl DynamicFilters for Mangadotnet {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut query_parameters = QueryParameters::new();

		if !hide_nsfw() {
			query_parameters.push("adult", Some("both"));
		}

		query_parameters.push("_routes", Some("pages/SearchPage"));

		let search_page: SearchPage = self
			.get_page_container_json_data(&format!("{BASE_URL}/search.data?{query_parameters}"))?;

		Ok(vec![Filter {
			id: Cow::from("genre"),
			title: Some("Genres".into()),
			hide_from_header: None,
			kind: FilterKind::MultiSelect {
				is_genre: true,
				can_exclude: true,
				uses_tag_style: true,
				options: search_page.all_genres.into_iter().map(Into::into).collect(),
				ids: None,
				default_included: None,
				default_excluded: None,
			},
		}])
	}
}

impl PageImageProcessor for Mangadotnet {
	fn process_page_image(
		&self,
		response: ImageResponse,
		_context: Option<PageContext>,
	) -> Result<ImageRef> {
		if !use_view_web_worker() {
			return Ok(response.image);
		}

		let Some(url) = response.request.url else {
			return Ok(response.image);
		};

		let base64_image_data = self.web_view.borrow_mut().fetch(&url, 0)?;
		let Some((_, base64_data)) = base64_image_data.split_once(',') else {
			bail!("Unable to get the raw image data")
		};
		let image_data = base64::engine::general_purpose::STANDARD
			.decode(base64_data)
			.or_else(|_| bail!("failed to decode image"))?;

		Ok(ImageRef::new(image_data.as_ref()))
	}
}

impl NotificationHandler for Mangadotnet {
	fn handle_notification(&self, notification: String) {
		if notification == NOTIFICATION_RESET_KEY {
			reset_filters();
		}
	}
}

register_source!(
	Mangadotnet,
	ListingProvider,
	Home,
	DeepLinkHandler,
	DynamicFilters,
	PageImageProcessor,
	NotificationHandler
);
