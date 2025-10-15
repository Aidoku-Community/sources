#![no_std]

use crate::models::{Archive, ArchiveMetadata, Category, SearchResult};
use aidoku::{
	BaseUrlProvider, Chapter, DynamicFilters, Filter, FilterValue, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, Result, SelectFilter, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};
use core::cell::RefCell;
mod models;
mod settings;

#[derive(Default)]
struct Lanraragi {
	current_start: RefCell<i32>,
}

impl Source for Lanraragi {
	fn new() -> Self {
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base_url = settings::get_base_url()?;
		let api_key = settings::get_api_key();

		let mut qs = QueryParameters::new();
		qs.push("sortby", Some("date_added"));
		qs.push("order", Some("desc"));

		let mut query_parts = Vec::new();
		if let Some(q) = query {
			query_parts.push(q);
		}

		for filter in filters {
			match filter {
				FilterValue::Text { value, .. } => query_parts.push(value),
				FilterValue::Select { id, value } => match id.as_str() {
					"category" => qs.set("category", Some(&value)),
					"genre" => query_parts.push(value),
					_ => {}
				},
				FilterValue::Sort {
					index, ascending, ..
				} => match index {
					0 => {
						qs.set("sortby", Some("date_added"));
						qs.set("order", Some(if ascending { "asc" } else { "desc" }));
					}
					1 => {
						qs.set("sortby", Some("title"));
						qs.set("order", Some(if ascending { "asc" } else { "desc" }));
					}
					2 => {
						qs.set("sortby", Some("lastread"));
						qs.set("order", Some(if ascending { "asc" } else { "desc" }));
					}
					_ => {}
				},
				_ => continue,
			}
		}

		if !query_parts.is_empty() {
			let combined_query = query_parts.join(" ");
			qs.set("filter", Some(&combined_query));
		}

		let start = *self.current_start.borrow();
		if page > 1 {
			qs.set("start", Some(&start.to_string()));
		}

		let url = format!("{}/api/search?{}", base_url, qs);

		let mut request = Request::get(&url)?;
		if !api_key.is_empty() {
			let encoded_key = base64::Engine::encode(
				&base64::engine::general_purpose::STANDARD,
				api_key.as_bytes(),
			);
			request = request.header("Authorization", &format!("Bearer {}", encoded_key));
		}
		let search_result: SearchResult = request.send()?.get_json()?;

		let archives_len = search_result.data.len();

		let current_result_count = archives_len as i32;

		*self.current_start.borrow_mut() += current_result_count;

		Ok(MangaPageResult {
			entries: search_result
				.data
				.into_iter()
				.map(|archive| archive.into_manga(&base_url))
				.collect(),
			has_next_page: start + current_result_count < search_result.records_filtered,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let base_url = settings::get_base_url()?;
		let api_key = settings::get_api_key();
		let url = format!("{}/api/archives/{}/metadata", base_url, manga.key);
		let mut request = Request::get(&url)?;
		if !api_key.is_empty() {
			let encoded_key = base64::Engine::encode(
				&base64::engine::general_purpose::STANDARD,
				api_key.as_bytes(),
			);
			request = request.header("Authorization", &format!("Bearer {}", encoded_key));
		}
		let archive: Archive = request.send()?.get_json()?;

		if needs_details {
			manga.title = archive.title;
			// Remove date_added and URL tags from display tags
			let display_tags: Vec<String> = archive
				.tags
				.split(',')
				.filter_map(|tag| {
					let tag = tag.trim();
					if tag.starts_with("date_added:") || tag.contains("://") {
						None
					} else {
						Some(tag.to_string())
					}
				})
				.collect();

			// Filter tags containing URLs for description
			let url_tags: Vec<String> = archive
				.tags
				.split(',')
				.filter_map(|tag| {
					let tag = tag.trim();
					if tag.contains("://") {
						Some(tag.to_string())
					} else {
						None
					}
				})
				.collect();

			manga.description = if url_tags.is_empty() {
				None
			} else {
				Some(url_tags.join("  \n"))
			};

			manga.cover = Some(format!(
				"{}/api/archives/{}/thumbnail",
				base_url, archive.arcid
			));
			manga.tags = Some(display_tags);
			manga.status = MangaStatus::Unknown;
			manga.update_strategy = aidoku::UpdateStrategy::Never;
		}

		if needs_chapters {
			// LANraragi archives typically have only one chapter
			let date_uploaded = archive.tags.split(',').find_map(|tag| {
				let tag = tag.trim();
				if tag.starts_with("date_added:") {
					tag.strip_prefix("date_added:")?.parse::<i64>().ok()
				} else {
					None
				}
			});

			let chapter = Chapter {
				key: archive.arcid.clone(),
				title: Some(format!("{} pages", archive.pagecount)),
				chapter_number: Some(1.0),
				date_uploaded,
				url: Some(format!("{}/reader?id={}", base_url, archive.arcid)),
				..Default::default()
			};
			manga.chapters = Some(vec![chapter]);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let base_url = settings::get_base_url()?;
		let api_key = settings::get_api_key();
		let url = format!("{}/api/archives/{}/files", base_url, chapter.key);

		let mut request = Request::get(&url)?;
		if !api_key.is_empty() {
			let encoded_key = base64::Engine::encode(
				&base64::engine::general_purpose::STANDARD,
				api_key.as_bytes(),
			);
			request = request.header("Authorization", &format!("Bearer {}", encoded_key));
		}
		let archive_metadata: ArchiveMetadata = request.send()?.get_json()?;

		let base_url = settings::get_base_url()?;
		Ok(archive_metadata
			.pages
			.into_iter()
			.map(|page_url| {
				let full_url = format!("{}{}", base_url, page_url);
				Page {
					content: PageContent::Url(full_url, None),
					..Default::default()
				}
			})
			.collect())
	}
}

impl BaseUrlProvider for Lanraragi {
	fn get_base_url(&self) -> Result<String> {
		settings::get_base_url()
	}
}

impl DynamicFilters for Lanraragi {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let base_url = settings::get_base_url()?;
		let api_key = settings::get_api_key();
		let url = format!("{}/api/categories", base_url);

		let mut request = Request::get(&url)?;
		if !api_key.is_empty() {
			let encoded_key = base64::Engine::encode(
				&base64::engine::general_purpose::STANDARD,
				api_key.as_bytes(),
			);
			request = request.header("Authorization", &format!("Bearer {}", encoded_key));
		}
		let categories: Vec<Category> = request.send()?.get_json()?;

		let mut options = vec!["All".to_string()];
		let mut values = vec!["".to_string()];

		let mut sorted_categories = categories;
		sorted_categories.sort_by(|a, b| {
			// Sort by pinned status first (pinned first)
			let a_pinned = a.pinned == "1";
			let b_pinned = b.pinned == "1";
			if a_pinned != b_pinned {
				return b_pinned.cmp(&a_pinned);
			}
			// Then sort by name
			a.name.cmp(&b.name)
		});

		for category in sorted_categories {
			let display_name = if category.pinned == "1" {
				format!("📌 {}", category.name)
			} else {
				category.name.clone()
			};
			options.push(display_name);
			values.push(category.id);
		}

		let categories = SelectFilter {
			id: "category".to_string().into(),
			title: Some("Category".to_string().into()),
			options: options.into_iter().map(|s| s.into()).collect(),
			ids: Some(values.into_iter().map(|s| s.into()).collect()),
			..Default::default()
		};

		Ok(vec![categories.into()])
	}
}

register_source!(Lanraragi, BaseUrlProvider, DynamicFilters);
