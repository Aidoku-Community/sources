#![no_std]
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	helpers::uri::encode_uri_component,
	imports::{error::AidokuError, net::Request},
	prelude::*,
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
};

mod home;
mod models;
mod settings;

use models::*;

const API_URL: &str = "https://nhentai.net/api";
const BASE_URL: &str = "https://nhentai.net";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604";

struct Nhentai;

impl Source for Nhentai {
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
		if let Some(q) = &query {
			if let Ok(id) = q.parse::<i32>() {
				let url = format!("{}/gallery/{}", API_URL, id);
				let gallery: NhentaiGallery = Request::get(&url)?
					.header("User-Agent", USER_AGENT)
					.send()?
					.get_json()?;
				let manga = gallery.into_manga();
				return Ok(MangaPageResult {
					entries: vec![manga],
					has_next_page: false,
				});
			}
		}

		let mut query_parts = Vec::new();

		// Add main query if present
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
						query_parts.push(format!("artist:{}", value));
					}
					"groups" => {
						query_parts.push(format!("group:{}", value));
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
					// nhentai doesn't use ascending parameter in the same way
				}
				FilterValue::MultiSelect { id, included, excluded, .. } => match id.as_str() {
					"tags" => {
						for tag in included {
							query_parts.push(format!("tag:\"{}\"", tag));
						}
						for tag in excluded {
							query_parts.push(format!("-tag:\"{}\"", tag));
						}
					}
					_ => continue,
				},
				_ => continue,
			}
		}

		let languages = settings::get_languages();
		for lang in languages {
			query_parts.push(format!("language:{}", lang));
		}

		let combined_query = if query_parts.is_empty() {
			" ".to_string()
		} else {
			query_parts.join(" ")
		};
		let url = format!(
			"{}/galleries/search?query={}&page={}&sort={}",
			API_URL,
			encode_uri_component(&combined_query),
			page,
			sort
		);

		let response: NhentaiSearchResponse = Request::get(&url)?
			.header("User-Agent", USER_AGENT)
			.send()?
			.get_json()?;

		let result_vec = response.result;
		let has_next_page = page < response.num_pages;

		let blocklist = settings::get_blocklist();

		let entries = result_vec
			.into_iter()
			.filter(|gallery| {
				if blocklist.is_empty() {
					return true;
				}
				!gallery.tags.iter().any(|tag| {
					blocklist.contains(&tag.name.to_lowercase())
				})
			})
			.map(|gallery| gallery.into_manga())
			.collect::<Vec<Manga>>();

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
			let url = format!("{}/gallery/{}", API_URL, manga.key);
			let gallery: NhentaiGallery = Request::get(&url)?
				.header("User-Agent", USER_AGENT)
				.send()?
				.get_json()?;

			if needs_details {
				manga.copy_from(gallery.clone().into_manga());
			}

			if needs_chapters {
				// nhentai galleries are single chapter
				let mut languages = Vec::new();
				for tag in &gallery.tags {
					if tag.r#type == "language" && tag.name != "translated" && tag.name != "rewrite" {
						languages.push(tag.name.clone());
					}
				}

				let chapter = Chapter {
					key: manga.key.clone(),
					title: Some(format!("{} pages", gallery.num_pages)),
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
		let gallery: NhentaiGallery = Request::get(&api_url)?
			.header("User-Agent", USER_AGENT)
			.send()?
			.get_json()?;

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

impl ListingProvider for Nhentai {
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

impl DeepLinkHandler for Nhentai {
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

register_source!(Nhentai, Home, ListingProvider, DeepLinkHandler);
