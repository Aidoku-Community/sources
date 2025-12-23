#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, Listing,
	ListingProvider, Manga, MangaPageResult, MultiSelectFilter, Page, PageContent, Result, Source,
	SortFilter, TextFilter,
	alloc::{String, Vec, borrow::Cow, string::ToString, vec},
	helpers::uri::encode_uri_component,
	imports::{error::AidokuError, net::Request},
	prelude::*,
};

mod home;
mod localization_cn;
mod models;
mod settings;
mod tags;

use models::*;
use tags::TAGS_EN;

/// Convert blocklist tags to English for matching
fn normalize_blocklist(blocklist: Vec<String>) -> Vec<String> {
	blocklist
		.into_iter()
		.map(|tag| reverse_translate_tag(&tag).to_lowercase())
		.collect()
}

const BASE_URL: &str = "https://nhentai.net";
const API_URL: &str = "https://nhentai.net/api";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) \
						  AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 \
						  Mobile/15E148 Safari/604";

struct NHentai;

impl Source for NHentai {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// If the query is a numeric ID, return the manga directly
		if let Some(q) = &query
			&& let Ok(id) = q.parse::<i32>()
		{
			let url = format!("{API_URL}/gallery/{id}");
			let gallery: NHentaiGallery = Request::get(&url)?
				.header("User-Agent", USER_AGENT)
				.json_owned()?;
			return Ok(MangaPageResult {
				entries: vec![gallery.into()],
				has_next_page: false,
			});
		}

		let mut query_parts = Vec::new();

		if let Some(q) = query {
			query_parts.push(q);
		}

		let mut sort = "recent";

		// parse filters
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" => {
						query_parts.push(value);
					}
					"artist" => {
						query_parts.push(format!("artist:{value}"));
					}
					"groups" => {
						query_parts.push(format!("group:{value}"));
					}
					_ => continue,
				},
				FilterValue::Sort { index, .. } => {
					sort = match index {
						0 => "recent",        // Latest
						1 => "popular-today", // Popular Today
						2 => "popular-week",  // Popular Week
						3 => "popular",       // Popular All
						_ => "recent",
					};
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
					..
				} => {
					if id == "tags" || id == "favorite_tags" {
						for tag in included {
							let eng_tag = reverse_translate_tag(&tag);
							query_parts.push(format!("tag:\"{eng_tag}\""));
						}
						for tag in excluded {
							let eng_tag = reverse_translate_tag(&tag);
							query_parts.push(format!("-tag:\"{eng_tag}\""));
						}
					}
				}
				FilterValue::Select { id, value } => {
					if id == "genre" {
						let eng_value = reverse_translate_tag(&value);
						query_parts.push(format!("tag:\"{eng_value}\""));
					}
				}
				_ => continue,
			}
		}

		if let Some(language) = settings::get_language() {
			query_parts.push(format!("language:{language}"));
		}

		let combined_query = if query_parts.is_empty() {
			" ".into()
		} else {
			query_parts.join(" ")
		};
		let url = format!(
			"{API_URL}/galleries/search?query={}&page={page}&sort={sort}",
			encode_uri_component(combined_query),
		);
		let response: NHentaiSearchResponse = Request::get(&url)?
			.header("User-Agent", USER_AGENT)
			.json_owned()?;

		let blocklist = normalize_blocklist(settings::get_blocklist());

		let entries = response
			.result
			.into_iter()
			.filter(|gallery| {
				if blocklist.is_empty() {
					return true;
				}
				!gallery
					.tags
					.iter()
					.any(|tag| blocklist.contains(&tag.name.to_lowercase()))
			})
			.map(|gallery| gallery.into())
			.collect::<Vec<Manga>>();
		let has_next_page = page < response.num_pages;

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
		if needs_details || needs_chapters {
			let url = format!("{API_URL}/gallery/{}", manga.key);
			let gallery: NHentaiGallery = Request::get(&url)?
				.header("User-Agent", USER_AGENT)
				.json_owned()?;

			if needs_details {
				manga.copy_from(gallery.clone().into());
			}

			if needs_chapters {
				// nhentai galleries are single chapter
				let mut languages = Vec::new();
				for tag in &gallery.tags {
					if tag.r#type == "language" && tag.name != "translated" && tag.name != "rewrite"
					{
						languages.push(tag.name.clone());
					}
				}

				let chapter = Chapter {
					key: manga.key.clone(),
					chapter_number: Some(1.0),
					date_uploaded: Some(gallery.upload_date),
					url: Some(format!("{}/g/{}", BASE_URL, manga.key)),
					scanlators: if !languages.is_empty() {
						Some(vec![languages.join(", ")])
					} else {
						None
					},
					..Default::default()
				};
				manga.chapters = Some(vec![chapter]);
			}
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let api_url = format!("{}/gallery/{}", API_URL, chapter.key);
		let gallery: NHentaiGallery = Request::get(&api_url)?
			.header("User-Agent", USER_AGENT)
			.json_owned()?;

		let pages = gallery
			.images
			.pages
			.iter()
			.enumerate()
			.map(|(i, page)| Page {
				content: PageContent::url(format!(
					"https://i.nhentai.net/galleries/{}/{}.{}",
					gallery.media_id,
					i + 1,
					extension_from_type(&page.t)
				)),
				..Default::default()
			})
			.collect::<Vec<Page>>();

		Ok(pages)
	}
}

impl ListingProvider for NHentai {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"popular-today" => self.get_search_manga_list(
				None,
				page,
				vec![FilterValue::Sort {
					id: "sort".to_string(),
					index: 1,
					ascending: false,
				}],
			),
			"popular-week" => self.get_search_manga_list(
				None,
				page,
				vec![FilterValue::Sort {
					id: "sort".to_string(),
					index: 2,
					ascending: false,
				}],
			),
			"popular" => self.get_search_manga_list(
				None,
				page,
				vec![FilterValue::Sort {
					id: "sort".to_string(),
					index: 3,
					ascending: false,
				}],
			),
			"latest" => self.get_search_manga_list(
				None,
				page,
				vec![FilterValue::Sort {
					id: "sort".to_string(),
					index: 0,
					ascending: false,
				}],
			),
			_ => Err(AidokuError::Unimplemented),
		}
	}
}

impl DeepLinkHandler for NHentai {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}

		const GALLERY_PATH: &str = "/g/";

		if let Some(id_start) = url.find(GALLERY_PATH) {
			let id_part = &url[id_start + GALLERY_PATH.len()..];
			let end = id_part.find('/').unwrap_or(id_part.len());
			let manga_id = &id_part[..end];

			Ok(Some(DeepLinkResult::Manga {
				key: manga_id.into(),
			}))
		} else {
			Ok(None)
		}
	}
}

impl DynamicFilters for NHentai {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let favorite_tags = settings::get_favorite_tags();
		let tag_lang = settings::get_tag_language();

		let mut filters: Vec<Filter> = Vec::new();

		// Artist filter
		filters.push(
			TextFilter {
				id: Cow::Borrowed("artist"),
				title: Some(Cow::Borrowed("Artist")),
				placeholder: Some(Cow::Borrowed("Artist name")),
				..Default::default()
			}
			.into(),
		);

		// Group filter
		filters.push(
			TextFilter {
				id: Cow::Borrowed("groups"),
				title: Some(Cow::Borrowed("Group")),
				placeholder: Some(Cow::Borrowed("Group name")),
				..Default::default()
			}
			.into(),
		);

		// Sort filter
		filters.push(
			SortFilter {
				id: Cow::Borrowed("sort"),
				title: Some(Cow::Borrowed("Sort")),
				can_ascend: false,
				options: vec![
					Cow::Borrowed("Latest"),
					Cow::Borrowed("Popular Today"),
					Cow::Borrowed("Popular Week"),
					Cow::Borrowed("Popular All"),
				],
				..Default::default()
			}
			.into(),
		);

		// Favorite tags filter (only show if user has favorite tags)
		if !favorite_tags.is_empty() {
			let translated_fav_tags: Vec<Cow<'static, str>> = if tag_lang != "english" {
				favorite_tags
					.iter()
					.map(|tag| Cow::Owned(translate_tag(tag, &tag_lang)))
					.collect()
			} else {
				favorite_tags.into_iter().map(Cow::Owned).collect()
			};

			filters.push(
				MultiSelectFilter {
					id: Cow::Borrowed("favorite_tags"),
					title: Some(Cow::Borrowed("Favorite Tags")),
					is_genre: true,
					can_exclude: true,
					uses_tag_style: true,
					options: translated_fav_tags,
					..Default::default()
				}
				.into(),
			);
		}

		// All tags filter - translate if needed
		let sort_alphabetically = settings::get_sort_tags_alphabetically();

		let mut all_tags: Vec<Cow<'static, str>> = if tag_lang != "english" {
			TAGS_EN
				.iter()
				.map(|tag| Cow::Owned(translate_tag(tag, &tag_lang)))
				.collect()
		} else {
			TAGS_EN.iter().map(|&s| Cow::Borrowed(s)).collect()
		};

		// Sort alphabetically if user enabled the option
		if sort_alphabetically {
			all_tags.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
		}

		filters.push(
			MultiSelectFilter {
				id: Cow::Borrowed("tags"),
				title: Some(Cow::Borrowed("Tags")),
				is_genre: true,
				can_exclude: true,
				uses_tag_style: true,
				options: all_tags,
				..Default::default()
			}
			.into(),
		);

		Ok(filters)
	}
}

register_source!(NHentai, Home, ListingProvider, DeepLinkHandler, DynamicFilters);
