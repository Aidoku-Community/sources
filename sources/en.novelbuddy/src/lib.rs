#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::std::send_partial_result,
	prelude::*,
};

mod helpers;
mod markdown;
mod models;
mod mtl;
mod settings;
mod watermark;

use helpers::{fetch_chapter_list, request, resolve_slug};
use markdown::to_markdown;
use models::{ChapterDetailData, ListData, TitleDetailData, TrendingData};
use settings::{
	content_rating_filter as persistent_content_rating, status_filter as persistent_status,
};

const CONTENT_RATING_IDS: &[&str] = &["safe", "suggestive", "nsfw"];

pub const BASE_URL: &str = "https://novelbuddy.me";
pub const API_URL: &str = "https://api.novelbuddy.me";
pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 \
	 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1";

const SORT_IDS: &[&str] = &[
	"popular",
	"latest",
	"newest",
	"rating",
	"views",
	"bookmarks",
];

const STATUS_IDS: &[&str] = &["ongoing", "completed", "hiatus", "cancelled"];

fn rating_matches(rating: &str, content_rating: aidoku::ContentRating) -> bool {
	matches!(
		(rating, content_rating),
		("safe", aidoku::ContentRating::Safe)
			| ("suggestive", aidoku::ContentRating::Suggestive)
			| ("nsfw", aidoku::ContentRating::NSFW)
	)
}

fn filter_by_rating(entries: Vec<Manga>, ratings: &[String]) -> Vec<Manga> {
	if ratings.is_empty() {
		return entries;
	}
	entries
		.into_iter()
		.filter(|m| ratings.iter().any(|r| rating_matches(r, m.content_rating)))
		.collect()
}

struct NovelBuddy;

impl Source for NovelBuddy {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		let mut has_sort = false;
		let mut excluded_genres = settings::hidden_genres();
		let mut status_filter: Vec<String> = Vec::new();
		let mut content_rating_filter: Vec<String> = Vec::new();

		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(s) = SORT_IDS.get(index as usize) {
						qs.push("sort", Some(*s));
						has_sort = true;
					}
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "status" => {
					let mut selected: Vec<String> = included.clone();
					if !excluded.is_empty() {
						let all: Vec<&str> = STATUS_IDS.to_vec();
						selected = all
							.iter()
							.filter(|s| !excluded.contains(&(**s).to_lowercase()))
							.map(|s| (*s).into())
							.collect();
					}
					status_filter = selected;
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "contentRating" => {
					let mut selected: Vec<String> = included.clone();
					if !excluded.is_empty() {
						let all: Vec<&str> = CONTENT_RATING_IDS.to_vec();
						selected = all
							.iter()
							.filter(|s| !excluded.contains(&(**s).to_lowercase()))
							.map(|s| (*s).into())
							.collect();
					}
					content_rating_filter = selected;
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genres" => {
					let included: Vec<String> = included
						.into_iter()
						.filter(|genre| {
							// if a hidden genre is manually included in filters, skip hiding it
							if let Some(pos) = excluded_genres.iter().position(|g| g == genre) {
								excluded_genres.swap_remove(pos);
								false
							} else {
								true
							}
						})
						.collect();
					if !included.is_empty() {
						qs.push("genres", Some(&included.join(",")));
					}
					for genre in excluded {
						if !excluded_genres.contains(&genre) {
							excluded_genres.push(genre);
						}
					}
				}
				_ => {}
			}
		}

		if !has_sort {
			qs.push("sort", Some("popular"));
		}
		if !excluded_genres.is_empty() {
			qs.push("exclude", Some(&excluded_genres.join(",")));
		}
		if let Some(q) = query.as_deref() {
			qs.push("q", Some(q));
		}

		let hide = settings::hide_mtl() && !query.as_deref().is_some_and(mtl::mentions_mtl);

		let mut all_ratings: Vec<String> = content_rating_filter;
		for r in persistent_content_rating() {
			if !all_ratings.contains(&r) {
				all_ratings.push(r);
			}
		}
		all_ratings.sort();
		all_ratings.dedup();

		let mut all_statuses: Vec<String> = status_filter;
		for s in persistent_status() {
			if !all_statuses.contains(&s) {
				all_statuses.push(s);
			}
		}
		all_statuses.sort();
		all_statuses.dedup();

		if all_statuses.len() > 1 {
			let mut all_entries: Vec<Manga> = Vec::new();
			for status in &all_statuses {
				let mut qs_status = QueryParameters::new();
				qs_status.push("sort", Some("popular"));
				if !excluded_genres.is_empty() {
					qs_status.push("exclude", Some(&excluded_genres.join(",")));
				}
				if let Some(q) = query.as_deref() {
					qs_status.push("q", Some(q));
				}
				qs_status.push("status", Some(status));
				qs_status.set("page", Some(&page.max(1).to_string()));
				let url = format!("{API_URL}/titles/search?{qs_status}");
				if let Ok(list_data) = request::<ListData>(&url) {
					let mut filtered: Vec<Manga> = list_data
						.items
						.into_iter()
						.filter(|item| item.status.as_deref() == Some(status))
						.map(Manga::from)
						.collect();
					all_entries.append(&mut filtered);
				}
			}
			if hide {
				mtl::retain_manga(&mut all_entries);
			}
			all_entries = filter_by_rating(all_entries, &all_ratings);
			all_entries.sort_by_key(|m| m.key.clone());
			all_entries.dedup_by_key(|m| m.key.clone());
			return Ok(MangaPageResult {
				entries: all_entries,
				has_next_page: false,
			});
		} else if let Some(status) = all_statuses.first() {
			qs.push("status", Some(status));
		}
		let url = format!("{API_URL}/titles/search?{qs}");
		let data: ListData = request(&url)?;
		let mut entries: Vec<Manga> = data.items.into_iter().map(Manga::from).collect();
		if hide {
			mtl::retain_manga(&mut entries);
		}
		entries = filter_by_rating(entries, &all_ratings);
		Ok(MangaPageResult {
			entries,
			has_next_page: data.pagination.has_next,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{API_URL}/titles/{}", manga.key);
			let data: TitleDetailData = request(&url)?;
			manga.copy_from(data.title.into());
			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			let mut chapters = fetch_chapter_list(&manga.key)?;
			if settings::hide_mtl() {
				mtl::retain_chapters(&mut chapters);
			}
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{API_URL}/titles/{}/chapters/{}", manga.key, chapter.key);
		let data: ChapterDetailData = request(&url)?;
		let body = data
			.chapter
			.content
			.as_deref()
			.map(to_markdown)
			.unwrap_or_default();
		let text = if body.is_empty() {
			"(empty chapter)".to_string()
		} else {
			body
		};
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl ListingProvider for NovelBuddy {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"trending-week" | "trending-today" => {
				let window = if listing.id == "trending-week" {
					"week"
				} else {
					"today"
				};
				let url = format!("{API_URL}/trending/titles?window={window}");
				let data: TrendingData = request(&url)?;
				let mut entries: Vec<Manga> = data.items.into_iter().map(Manga::from).collect();
				if settings::hide_mtl() {
					mtl::retain_manga(&mut entries);
				}
				entries = filter_by_rating(entries, &persistent_content_rating());
				Ok(MangaPageResult {
					entries,
					has_next_page: false,
				})
			}
			"latest" => {
				let url = format!("{API_URL}/titles/search?sort=latest&page={}", page.max(1));
				let data: ListData = request(&url)?;
				let mut entries: Vec<Manga> = data.items.into_iter().map(Manga::from).collect();
				if settings::hide_mtl() {
					mtl::retain_manga(&mut entries);
				}
				entries = filter_by_rating(entries, &persistent_content_rating());
				Ok(MangaPageResult {
					entries,
					has_next_page: data.pagination.has_next,
				})
			}
			_ => {
				bail!("Unknown listing: {}", listing.id);
			}
		}
	}
}

impl DeepLinkHandler for NovelBuddy {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split(['?', '#'])
			.next()
			.unwrap_or(&url)
			.rsplit("novelbuddy.me")
			.next()
			.unwrap_or("")
			.trim_start_matches('/');
		if path.is_empty() {
			return Ok(None);
		}
		let mut parts = path.splitn(2, '/');
		let series_slug = match parts.next() {
			Some(s) if !s.is_empty() => s,
			_ => return Ok(None),
		};
		let chapter_slug = parts.next();

		let manga_key = resolve_slug(series_slug)?;

		if let Some(ch_slug) = chapter_slug
			&& !ch_slug.is_empty()
		{
			let chapters = fetch_chapter_list(&manga_key)?;
			let chapter_key = chapters.into_iter().find_map(|c| {
				let url = c.url.as_deref().unwrap_or("");
				if url.ends_with(ch_slug) {
					Some(c.key)
				} else {
					None
				}
			});
			if let Some(key) = chapter_key {
				return Ok(Some(DeepLinkResult::Chapter { manga_key, key }));
			}
		}
		Ok(Some(DeepLinkResult::Manga { key: manga_key }))
	}
}

register_source!(NovelBuddy, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn search_returns_results() {
		let source = NovelBuddy;
		let result = source
			.get_search_manga_list(Some("shadow slave".into()), 1, Vec::new())
			.expect("search failed");
		assert!(!result.entries.is_empty(), "expected at least one result");
		assert!(
			result
				.entries
				.iter()
				.any(|m| m.title.to_lowercase().contains("shadow slave")),
			"expected 'Shadow Slave' in results"
		);
	}

	#[aidoku_test]
	fn series_detail_has_many_chapters() {
		let source = NovelBuddy;
		let manga = Manga {
			key: "VYPGVZ8z".into(),
			..Default::default()
		};
		let manga = source
			.get_manga_update(manga, true, true)
			.expect("get_manga_update failed");
		assert_eq!(manga.title, "Shadow Slave");
		assert!(manga.description.is_some());
		assert!(manga.authors.is_some());
		let chapters = manga.chapters.expect("no chapters returned");
		assert!(
			chapters.len() > 100,
			"expected >100 chapters, got {}",
			chapters.len()
		);
		// The ISO-8601 format string must keep matching the API's timestamps;
		// a silent mismatch would leave every chapter without an upload date.
		assert!(
			chapters.iter().any(|c| c.date_uploaded.is_some()),
			"expected at least one parsed upload date"
		);
	}

	#[aidoku_test]
	fn page_list_returns_text_page() {
		let source = NovelBuddy;
		let manga = Manga {
			key: "VYPGVZ8z".into(),
			..Default::default()
		};
		// Resolve a chapter key from the live list rather than hardcoding
		// one: the site occasionally removes or re-publishes chapters.
		let chapters = fetch_chapter_list(&manga.key).expect("chapter list failed");
		let key = chapters
			.last()
			.map(|c| c.key.clone())
			.expect("no chapters returned");
		let chapter = Chapter {
			key,
			..Default::default()
		};
		let pages = source
			.get_page_list(manga, chapter)
			.expect("get_page_list failed");
		assert_eq!(pages.len(), 1);
		match &pages[0].content {
			PageContent::Text(text) => {
				assert!(!text.is_empty(), "expected non-empty chapter text");
			}
			_ => panic!("expected PageContent::Text"),
		}
	}

	#[aidoku_test]
	fn deep_link_resolves_series() {
		let source = NovelBuddy;
		let result = source
			.handle_deep_link("https://novelbuddy.me/shadow-slave".into())
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Manga { key } => assert_eq!(key, "VYPGVZ8z"),
			_ => panic!("expected Manga deep link"),
		}
	}

	#[aidoku_test]
	fn status_multi_select_finds_results() {
		use aidoku::alloc::vec;
		let filters = [FilterValue::MultiSelect {
			id: "status".into(),
			included: vec!["ongoing".into(), "completed".into()],
			excluded: Vec::new(),
		}];
		let result = NovelBuddy
			.get_search_manga_list(Some("shadow slave".into()), 1, filters)
			.expect("search with status filter failed");
		assert!(
			!result.entries.is_empty(),
			"status multi-select should return results, got {}",
			result.entries.len()
		);
	}
}
