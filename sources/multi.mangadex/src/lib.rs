#![no_std]
use aidoku::{
	AlternateCoverProvider, Chapter, DeepLinkHandler, DeepLinkResult, DynamicListings, FilterValue,
	Listing, ListingKind, ListingProvider, Manga, MangaPageResult, Page, PageContent, Result,
	Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		error::AidokuError,
		net::{Request, TimeUnit, set_rate_limit},
		std::send_partial_result,
	},
	prelude::*,
};
use core::fmt::Write;
use hashbrown::HashSet;

mod auth;
mod models;
mod settings;

mod home;

use auth::*;
use models::*;

const API_URL: &str = "https://api.mangadex.org";
const COVER_URL: &str = "https://uploads.mangadex.org";

const PAGE_SIZE: i32 = 20;
const CUSTOM_LIST_PREFIX: &str = "list-";

// listings to use on the home page
const CUSTOM_LISTS: &[&str] = &[
	"f66ebc10-ef89-46d1-be96-bb704559e04a", // Self-Published
	"805ba886-dd99-4aa4-b460-4bd7c7b71352", // Staff Picks
	"5c5e6e39-0b4b-413e-be59-27b1ba03d1b9", // Featured by Supporters
];

struct MangaDex;

impl Source for MangaDex {
	fn new() -> Self {
		// 5 requests per second (https://api.mangadex.org/docs/2-limitations/)
		set_rate_limit(5, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let offset = (page - 1) * PAGE_SIZE;

		let mut qs = QueryParameters::new();

		let mut use_default_content_rating = true;
		let mut has_available_chapters = true;

		// parse filters
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" | "artist" => {
						let ids = self.get_author_ids(&value)?;
						if let Some(first) = ids.first() {
							qs.push("authorOrArtist", Some(first));
						}
					}
					_ => return Err(AidokuError::Message("Invalid text filter id".into())),
				},
				FilterValue::Sort {
					index, ascending, ..
				} => {
					let key = format!(
						"order[{}]",
						match index {
							0 => "latestUploadedChapter",
							1 => "relevance",
							2 => "followedCount",
							3 => "createdAt",
							4 => "updatedAt",
							5 => "title",
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
					"lang" => {
						for id in included {
							id.split(",").for_each(|lang| {
								qs.push("originalLanguage[]", Some(lang));
							});
						}
						for id in excluded {
							id.split(",").for_each(|lang| {
								qs.push("excludedOriginalLanguage[]", Some(lang));
							});
						}
					}
					"rating" => {
						use_default_content_rating = false;
						for id in included {
							qs.push("contentRating[]", Some(&id));
						}
					}
					"status" => {
						for id in included {
							qs.push("status[]", Some(&id));
						}
					}
					"demographic" => {
						for id in included {
							qs.push("publicationDemographic[]", Some(&id));
						}
					}
					_ => {
						for id in included {
							qs.push("includedTags[]", Some(&id));
						}
						for id in excluded {
							qs.push("excludedTags[]", Some(&id));
						}
					}
				},
				// has available chapters toggle
				FilterValue::Check { value: 0, .. } => has_available_chapters = false,
				// includedTagsMode and excludedTagsMode
				FilterValue::Select { id, value } => {
					qs.push(&id, Some(&value));
				}
				_ => continue,
			}
		}

		if let Some(query) = query {
			qs.push("title", Some(&query));
		}

		if use_default_content_rating {
			let default_ratings = settings::get_content_ratings_list()?;
			for rating in default_ratings {
				qs.push("contentRating[]", Some(&rating));
			}
		}

		if has_available_chapters {
			qs.push("hasAvailableChapters", Some("true"));
			let languages = settings::get_languages()?;
			for lang in languages {
				qs.push("availableTranslatedLanguage[]", Some(&lang));
			}
		}

		let url = format!(
			"{API_URL}/manga\
				?includes%5B%5D=cover_art\
				&limit={PAGE_SIZE}\
				&offset={offset}\
				&{qs}",
		);

		let (entries, has_next_page) = Request::get(url)?
			.send()?
			.get_json::<DexResponse<Vec<DexManga>>>()
			.map(|response| {
				(
					response
						.data
						.into_iter()
						.map(|value| value.into_basic_manga())
						.collect::<Vec<Manga>>(),
					response.total.is_some_and(|t| offset + PAGE_SIZE < t),
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
		if needs_details {
			manga.copy_from(
				Request::get(format!(
					"{API_URL}/manga/{}\
						?includes[]=cover_art\
						&includes[]=author\
						&includes[]=artist",
					manga.key
				))?
				.send()?
				.get_json::<DexResponse<DexManga>>()?
				.data
				.into(),
			);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let languages = settings::get_languages_with_key("translatedLanguage")?;
			let blocked_groups = settings::get_blocked_uuids()?;
			let show_unavailable_chapters = settings::get_locked_chapters();

			let url = format!(
				"{API_URL}/manga/{}/feed\
					?order[volume]=desc\
					&order[chapter]=desc\
					&limit=500\
					&contentRating[]=pornographic\
					&contentRating[]=erotica\
					&contentRating[]=suggestive\
					&contentRating[]=safe\
					&includeUnavailable={}\
					&includes[]=user\
					&includes[]=scanlation_group\
					{languages}\
					{blocked_groups}",
				manga.key,
				if show_unavailable_chapters { "1" } else { "0" }
			);

			let (mut chapters, total) = Request::get(&url)?
				.send()?
				.get_json::<DexResponse<Vec<DexChapter>>>()
				.map(|response| {
					(
						response
							.data
							.into_iter()
							.filter(|value| !value.has_external_url())
							.map(|value| value.into())
							.collect::<Vec<Chapter>>(),
						response.total,
					)
				})?;

			// fetch chapters in pages of 500
			if let Some(total) = total {
				let mut offset = 500;
				while offset < total {
					let url = format!("{url}&offset={offset}");
					if let Ok(response) = Request::get(&url)?
						.send()?
						.get_json::<DexResponse<Vec<DexChapter>>>()
					{
						chapters.extend(
							response
								.data
								.into_iter()
								.filter(|value| !value.has_external_url())
								.map(|value| value.into()),
						);
					}
					offset += 500;
				}
			}

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let force_port = settings::get_force_port();
		let url = format!(
			"{API_URL}/at-home/server/{}{}",
			chapter.key,
			if force_port { "?forcePort443=true" } else { "" }
		);

		Request::get(&url)?
			.send()?
			.get_json::<DexAtHomeResponse>()
			.and_then(|response| {
				let data_saver = settings::get_data_saver();
				let base_url = format!(
					"{}/{}/{}",
					response.base_url,
					if data_saver { "data-saver" } else { "data" },
					response.chapter.hash
				);

				let chapter_data = if data_saver {
					response.chapter.data_saver
				} else {
					response.chapter.data
				};
				chapter_data
					.map(|data| {
						data.iter()
							.map(|value| Page {
								content: PageContent::url(format!("{base_url}/{value}")),
								..Default::default()
							})
							.collect::<Vec<Page>>()
					})
					.ok_or(AidokuError::message("Missing chapter data"))
			})
	}
}

impl ListingProvider for MangaDex {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"recent" => self.get_search_manga_list(
				None,
				page,
				vec![FilterValue::Sort {
					id: String::default(),
					index: 3,
					ascending: false,
				}],
			),
			"popular" => self.get_search_manga_list(
				None,
				page,
				vec![FilterValue::Sort {
					id: String::default(),
					index: 2,
					ascending: false,
				}],
			),
			"latest" => self.get_latest_manga(page),
			"library" => self.get_library(page),
			_ if listing.id.starts_with(CUSTOM_LIST_PREFIX) => {
				self.get_mangadex_list(&listing.id[CUSTOM_LIST_PREFIX.len()..])
			}
			_ => Err(AidokuError::Unimplemented),
		}
	}
}

impl MangaDex {
	// get a list of author ids from a name query
	fn get_author_ids(&self, name: &str) -> Result<Vec<String>> {
		let url = format!("{API_URL}/author?name={name}",);

		let ids = Request::get(url)?
			.send()?
			.get_json::<DexResponse<Vec<DexData>>>()?
			.data
			.iter()
			.map(|value| value.id.to_string())
			.collect::<Vec<String>>();

		Ok(ids)
	}

	// get a custom list
	fn get_mangadex_list(&self, id: &str) -> Result<MangaPageResult> {
		let content_ratings = settings::get_content_ratings()?;

		let mut list_res = Request::get(format!("{API_URL}/list/{id}"))?.send()?;

		let manga_ids = list_res
			.get_json::<DexResponse<DexCustomList>>()?
			.data
			.relationships
			.iter()
			.filter_map(|relationship| {
				if relationship.r#type == "manga" {
					Some(relationship.id)
				} else {
					None
				}
			})
			.collect::<Vec<&str>>();

		// assume the list is 32 items or less (mangadex site uses this value)
		let entries = Request::get(format!(
			"{API_URL}/manga\
					?limit=100\
					&includes[]=cover_art\
					{content_ratings}\
					&ids[]={}",
			manga_ids.join("&ids[]=")
		))?
		.send()?
		.get_json::<DexResponse<Vec<DexManga>>>()
		.map(|response| {
			response
				.data
				.into_iter()
				.map(|value| value.into_basic_manga())
				.collect::<Vec<Manga>>()
		})?;

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	// get the manga associated with the latest uploaded chapters
	fn get_latest_manga(&self, page: i32) -> Result<MangaPageResult> {
		let languages = settings::get_languages_with_key("translatedLanguage")?;
		let content_ratings = settings::get_content_ratings()?;

		let offset = (page - 1) * PAGE_SIZE;

		let mut chapters_res = Request::get(format!(
			"{API_URL}/chapter\
				?includes[]=scanlation_group\
				&limit={PAGE_SIZE}\
				&offset={offset}\
				&order[readableAt]=desc\
				{content_ratings}\
				{languages}"
		))?
		.send()?; // get_data instead of json so that we can use it as a reference

		// get unique manga ids for the chapters
		let mut seen = HashSet::new();
		let manga_ids: Vec<&str> = chapters_res
			.get_json::<DexResponse<Vec<DexChapter>>>()?
			.data
			.into_iter()
			.filter_map(|chapter| chapter.manga_id())
			.filter(|&id| seen.insert(id))
			.collect();

		let has_next_page = !manga_ids.is_empty();

		let ids_params = manga_ids.iter().fold(String::new(), |mut output, id| {
			let _ = write!(output, "&ids[]={id}");
			output
		});

		let url = format!(
			"{API_URL}/manga\
				?includes[]=cover_art\
				{content_ratings}\
				{ids_params}"
		);
		let entries = Request::get(url)?
			.send()?
			.get_json::<DexResponse<Vec<DexManga>>>()?
			.data
			.into_iter()
			.map(|value| value.into_basic_manga())
			.collect::<Vec<Manga>>();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	// get the logged in user's library
	fn get_library(&self, page: i32) -> Result<MangaPageResult> {
		let status_ids = Request::get(format!("{API_URL}/manga/status"))?
			.authed_send()?
			.get_json::<DexStatusResponse>()?
			.statuses
			.keys()
			.fold(String::new(), |mut output, id| {
				let _ = write!(output, "&ids[]={id}");
				output
			});

		let offset = (page - 1) * PAGE_SIZE;

		let manga_url = format!(
			"{API_URL}/manga?\
				&limit={PAGE_SIZE}\
				&offset={offset}\
				&includes[]=cover_art\
				&contentRating[]=safe\
				&contentRating[]=suggestive\
				&contentRating[]=erotica\
				&contentRating[]=pornographic\
				{status_ids}",
		);

		let (entries, has_next_page) = Request::get(manga_url)?
			.authed_send()?
			.get_json::<DexResponse<Vec<DexManga>>>()
			.map(|response| {
				(
					response
						.data
						.into_iter()
						.map(|value| value.into_basic_manga())
						.collect::<Vec<Manga>>(),
					response.total.is_some_and(|t| offset + PAGE_SIZE < t),
				)
			})?;

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

// show the library listing if we're logged in
impl DynamicListings for MangaDex {
	fn get_dynamic_listings(&self) -> Result<Vec<Listing>> {
		if settings::is_logged_in() {
			Ok(Vec::from([Listing {
				id: String::from("library"),
				name: String::from("Library"),
				kind: ListingKind::Default,
			}]))
		} else {
			Ok(Vec::new())
		}
	}
}

impl AlternateCoverProvider for MangaDex {
	fn get_alternate_covers(&self, manga: Manga) -> Result<Vec<String>> {
		let locales = settings::get_languages_with_key("locales")?;

		let url = format!(
			"{API_URL}/cover?manga[]={}{}{}&limit=100",
			manga.key,
			locales,
			if !locales.contains("locales[]=ja") {
				"&locales[]=ja"
			} else {
				""
			}
		);
		let (mut items, total) = Request::get(&url)?
			.send()?
			.get_json::<DexResponse<Vec<DexCoverArt>>>()
			.map(|response| (response.data, response.total))?;

		if let Some(total) = total {
			let mut offset = 100;
			while offset < total {
				let url = format!("{url}&offset={offset}");
				if let Ok(response) = Request::get(url)?
					.send()?
					.get_json::<DexResponse<Vec<DexCoverArt>>>()
				{
					items.extend(response.data);
				}
				offset += 100;
			}
		}

		items.sort_by(|a, b| {
			let num_a = a
				.attributes
				.volume
				.as_ref()
				.and_then(|a| a.parse::<f32>().ok())
				.unwrap_or(0.0);
			let num_b = b
				.attributes
				.volume
				.as_ref()
				.and_then(|b| b.parse::<f32>().ok())
				.unwrap_or(0.0);
			num_b.total_cmp(&num_a)
		});

		let result = items
			.iter()
			.map(|item| {
				format!(
					"{COVER_URL}/covers/{}/{}{}",
					manga.key,
					item.attributes.file_name,
					settings::get_cover_quality()
				)
			})
			.collect::<Vec<String>>();

		Ok(result)
	}
}

impl DeepLinkHandler for MangaDex {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		const BASE_URL: &str = "https://mangadex.org/";

		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}

		let url = &url[BASE_URL.len()..]; // remove base url prefix

		const TITLE_PATH: &str = "title/";
		const CHAPTER_PATH: &str = "chapter/";

		if let Some(key) = url.strip_prefix(TITLE_PATH) {
			// ex: https://mangadex.org/title/a96676e5-8ae2-425e-b549-7f15dd34a6d8/komi-san-wa-komyushou-desu
			let end = key.find('/').unwrap_or(key.len());
			let manga_key = &key[..end];

			Ok(Some(DeepLinkResult::Manga {
				key: manga_key.into(),
			}))
		} else if let Some(key) = url.strip_prefix(CHAPTER_PATH) {
			// ex: https://mangadex.org/chapter/56eecc6f-1a4e-464c-b6a4-a1cbdf
			let end = key.find('/').unwrap_or(key.len());
			let chapter_key = &key[..end];

			let url = format!("{API_URL}/chapter/{chapter_key}");
			let mut res = Request::get(&url)?.send()?;

			let manga_key = res
				.get_json::<DexResponse<DexChapter>>()?
				.data
				.manga_id()
				.ok_or(AidokuError::message("Missing manga key"))?;

			Ok(Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: chapter_key.into(),
			}))
		} else {
			Ok(None)
		}
	}
}

register_source!(
	MangaDex,
	Home,
	ListingProvider,
	DynamicListings,
	AlternateCoverProvider,
	DeepLinkHandler
);
