#![no_std]
use aidoku::{
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider,
	Manga, MangaPageResult, Page, PageContent, Result, Source,
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

struct SearchContext {
	qs: QueryParameters,
	status_filter: Option<Vec<String>>,
	content_rating_filter: Option<Vec<String>>,
}

fn resolve_multi_select(
	all_options: &[&'static str],
	included: Vec<String>,
	excluded: Vec<String>,
) -> Option<Vec<String>> {
	let lower_excluded: Vec<String> = excluded.iter().map(|e| e.to_lowercase()).collect();
	let base: Vec<String> = if included.is_empty() {
		all_options.iter().map(|s| (*s).into()).collect()
	} else {
		included
	};
	let filtered: Vec<String> = base
		.into_iter()
		.filter(|s| !lower_excluded.contains(&s.to_lowercase()))
		.collect();
	Some(filtered)
}

fn parse_filters(filters: Vec<FilterValue>) -> SearchContext {
	let mut qs = QueryParameters::new();
	let mut has_sort = false;
	let mut excluded_genres = settings::hidden_genres();
	let mut status_filter: Option<Vec<String>> = None;
	let mut content_rating_filter: Option<Vec<String>> = None;

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
				status_filter = resolve_multi_select(STATUS_IDS, included, excluded);
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} if id == "contentRating" => {
				content_rating_filter =
					resolve_multi_select(CONTENT_RATING_IDS, included, excluded);
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} if id == "genres" => {
				for genre in &included {
					if let Some(pos) = excluded_genres.iter().position(|g| g == genre) {
						excluded_genres.swap_remove(pos);
					}
				}
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

	SearchContext {
		qs,
		status_filter,
		content_rating_filter,
	}
}

fn build_search_url(
	ctx: &SearchContext,
	query: Option<&str>,
	page: i32,
	status: Option<&str>,
) -> String {
	let mut qs = ctx.qs.clone();
	if let Some(q) = query {
		qs.push("q", Some(q));
	}
	if let Some(s) = status {
		qs.push("status", Some(s));
	}
	format!("{API_URL}/titles/search?{qs}&page={}", page.max(1))
}

fn merge_with_persistent(runtime: Option<Vec<String>>, persistent: Vec<String>) -> Vec<String> {
	match runtime {
		None => persistent,
		Some(mut values) => {
			values.sort();
			values.dedup();
			values
		}
	}
}

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

const SERVER_STATUS_IDS: &[&str] = &["ongoing", "completed"];

fn is_server_status(value: &str) -> bool {
	SERVER_STATUS_IDS
		.iter()
		.any(|supported| value.eq_ignore_ascii_case(supported))
}

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

fn fetch_multi_status(
	ctx: &SearchContext,
	query: Option<&str>,
	page: i32,
	all_statuses: &[String],
	all_ratings: &[String],
	hide: bool,
) -> Result<MangaPageResult> {
	let mut all_entries: Vec<Manga> = Vec::new();
	let mut any_has_next = false;
	let mut first_error: Option<AidokuError> = None;
	let mut success_count = 0usize;

	for status in all_statuses {
		let url = build_search_url(ctx, query, page, Some(status));
		match request::<ListData>(&url) {
			Ok(list_data) => {
				let mut filtered: Vec<Manga> = list_data
					.items
					.into_iter()
					.filter(|item| {
						item.status
							.as_deref()
							.is_some_and(|s| s.eq_ignore_ascii_case(status))
					})
					.map(Manga::from)
					.collect();
				if list_data.pagination.has_next {
					any_has_next = true;
				}
				all_entries.append(&mut filtered);
				success_count += 1;
			}
			Err(e) => {
				if first_error.is_none() {
					first_error = Some(e);
				}
			}
		}
	}

	if success_count == 0 {
		if let Some(e) = first_error {
			return Err(e);
		}
		bail!("all status requests failed");
	}

	if hide {
		mtl::retain_manga(&mut all_entries);
	}
	let mut entries = filter_by_rating(all_entries, all_ratings);
	let mut seen: Vec<String> = Vec::new();
	entries.retain(|m| {
		if seen.contains(&m.key) {
			false
		} else {
			seen.push(m.key.clone());
			true
		}
	});
	Ok(MangaPageResult {
		entries,
		has_next_page: any_has_next,
	})
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
		let ctx = parse_filters(filters);
		let hide = settings::hide_mtl() && !query.as_deref().is_some_and(mtl::mentions_mtl);

		let all_ratings = merge_with_persistent(
			ctx.content_rating_filter.clone(),
			persistent_content_rating(),
		);
		let all_statuses = merge_with_persistent(ctx.status_filter.clone(), persistent_status());

		let match_nothing = ctx
			.content_rating_filter
			.as_ref()
			.is_some_and(|ratings| ratings.is_empty())
			|| ctx
				.status_filter
				.as_ref()
				.is_some_and(|statuses| statuses.is_empty());
		if match_nothing {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		}

		if all_statuses.iter().any(|s| !is_server_status(s)) {
			let url = build_search_url(&ctx, query.as_deref(), page, None);
			let data: ListData = request(&url)?;
			let mut entries: Vec<Manga> = data
				.items
				.into_iter()
				.filter(|item| {
					item.status.as_deref().is_some_and(|status| {
						all_statuses
							.iter()
							.any(|selected| selected.eq_ignore_ascii_case(status))
					})
				})
				.map(Manga::from)
				.collect();
			if hide {
				mtl::retain_manga(&mut entries);
			}
			entries = filter_by_rating(entries, &all_ratings);
			return Ok(MangaPageResult {
				entries,
				has_next_page: data.pagination.has_next,
			});
		}

		if all_statuses.len() > 1 {
			return fetch_multi_status(
				&ctx,
				query.as_deref(),
				page,
				&all_statuses,
				&all_ratings,
				hide,
			);
		}

		let status = all_statuses.first().map(|s| s.as_str());
		let url = build_search_url(&ctx, query.as_deref(), page, status);
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

fn deep_link_slugs(path: &str) -> (&str, &str) {
	let mut segments = path.split('/').filter(|s| !s.is_empty());
	let series = segments.next().unwrap_or("");
	let chapter = segments.next().unwrap_or("");
	(series, chapter)
}

fn chapter_url_matches(url: &str, chapter_slug: &str) -> bool {
	!chapter_slug.is_empty() && url.trim_end_matches('/').rsplit('/').next() == Some(chapter_slug)
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
			.trim_start_matches('/')
			.trim_end_matches('/');
		if path.is_empty() {
			return Ok(None);
		}
		let (series_slug, chapter_slug) = deep_link_slugs(path);
		if series_slug.is_empty() {
			return Ok(None);
		}

		let manga_key = resolve_slug(series_slug)?;

		if !chapter_slug.is_empty() {
			let chapters = fetch_chapter_list(&manga_key)?;
			let chapter_key = chapters.into_iter().find_map(|c| {
				let url = c.url.as_deref().unwrap_or("");
				if chapter_url_matches(url, chapter_slug) {
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
	use aidoku::{ContentRating, FilterValue};
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn rating_matches_safe() {
		assert!(rating_matches("safe", ContentRating::Safe));
	}

	#[aidoku_test]
	fn rating_matches_suggestive() {
		assert!(rating_matches("suggestive", ContentRating::Suggestive));
	}

	#[aidoku_test]
	fn rating_matches_nsfw() {
		assert!(rating_matches("nsfw", ContentRating::NSFW));
	}

	#[aidoku_test]
	fn rating_matches_mismatched() {
		assert!(!rating_matches("safe", ContentRating::Suggestive));
		assert!(!rating_matches("nsfw", ContentRating::Safe));
		assert!(!rating_matches("suggestive", ContentRating::NSFW));
	}

	#[aidoku_test]
	fn filter_by_rating_empty_selection_returns_all() {
		let entries = vec![
			Manga {
				key: "1".into(),
				content_rating: ContentRating::Safe,
				..Default::default()
			},
			Manga {
				key: "2".into(),
				content_rating: ContentRating::NSFW,
				..Default::default()
			},
		];
		assert_eq!(filter_by_rating(entries, &[]).len(), 2);
	}

	#[aidoku_test]
	fn filter_by_rating_single_selection() {
		let entries = vec![
			Manga {
				key: "1".into(),
				content_rating: ContentRating::Safe,
				..Default::default()
			},
			Manga {
				key: "2".into(),
				content_rating: ContentRating::Suggestive,
				..Default::default()
			},
		];
		let filtered = filter_by_rating(entries, &["suggestive".into()]);
		assert_eq!(filtered.len(), 1);
		assert_eq!(filtered[0].key, "2");
	}

	#[aidoku_test]
	fn filter_by_rating_suggestive_and_nsfw_selection() {
		let entries = vec![
			Manga {
				key: "1".into(),
				content_rating: ContentRating::Safe,
				..Default::default()
			},
			Manga {
				key: "2".into(),
				content_rating: ContentRating::Suggestive,
				..Default::default()
			},
			Manga {
				key: "3".into(),
				content_rating: ContentRating::NSFW,
				..Default::default()
			},
		];
		let filtered = filter_by_rating(entries, &["suggestive".into(), "nsfw".into()]);
		assert_eq!(filtered.len(), 2);
	}

	#[aidoku_test]
	fn resolve_multi_select_empty_included_takes_all_options() {
		let selected =
			resolve_multi_select(&["safe", "suggestive", "nsfw"], Vec::new(), Vec::new()).unwrap();
		assert_eq!(selected, vec!["safe", "suggestive", "nsfw"]);
	}

	#[aidoku_test]
	fn resolve_multi_select_keeps_included_without_exclusions() {
		let selected = resolve_multi_select(
			&["safe", "suggestive", "nsfw"],
			vec!["nsfw".into()],
			Vec::new(),
		)
		.unwrap();
		assert_eq!(selected, vec!["nsfw"]);
	}

	#[aidoku_test]
	fn resolve_multi_select_applies_exclusions_to_included() {
		let selected = resolve_multi_select(
			&["safe", "suggestive", "nsfw"],
			vec!["safe".into(), "nsfw".into()],
			vec!["NSFW".into()],
		)
		.unwrap();
		assert_eq!(selected, vec!["safe"]);
	}

	#[aidoku_test]
	fn resolve_multi_select_excluding_every_option_yields_empty_selection() {
		let excluded: Vec<String> = ["safe", "suggestive", "nsfw"]
			.iter()
			.map(|s| (*s).into())
			.collect();
		let selected = resolve_multi_select(&["safe", "suggestive", "nsfw"], Vec::new(), excluded);
		assert_eq!(selected, Some(Vec::new()));
	}

	#[aidoku_test]
	fn merge_with_persistent_falls_back_when_no_runtime_selection() {
		assert_eq!(
			merge_with_persistent(None, vec!["safe".into()]),
			vec!["safe"]
		);
	}

	#[aidoku_test]
	fn merge_with_persistent_runtime_overrides_persistent() {
		assert_eq!(
			merge_with_persistent(Some(vec!["nsfw".into()]), vec!["safe".into()]),
			vec!["nsfw"]
		);
	}

	#[aidoku_test]
	fn merge_with_persistent_keeps_explicit_empty_selection() {
		assert_eq!(
			merge_with_persistent(Some(Vec::new()), vec!["safe".into()]),
			Vec::<String>::new()
		);
	}

	#[aidoku_test]
	fn server_status_rejects_unsupported_values() {
		assert!(is_server_status("ongoing"));
		assert!(is_server_status("Completed"));
		assert!(!is_server_status("hiatus"));
		assert!(!is_server_status("cancelled"));
	}

	#[aidoku_test]
	fn deep_link_slugs_take_first_two_segments() {
		assert_eq!(
			deep_link_slugs("shadow-slave/chapter-1/extra"),
			("shadow-slave", "chapter-1")
		);
		assert_eq!(deep_link_slugs("shadow-slave"), ("shadow-slave", ""));
		assert_eq!(deep_link_slugs(""), ("", ""));
	}

	#[aidoku_test]
	fn chapter_url_matches_final_segment_only() {
		assert!(chapter_url_matches(
			"https://novelbuddy.me/shadow-slave/chapter-1",
			"chapter-1"
		));
		assert!(chapter_url_matches(
			"https://novelbuddy.me/shadow-slave/chapter-1/",
			"chapter-1"
		));
		assert!(!chapter_url_matches(
			"https://novelbuddy.me/shadow-slave/chapter-10",
			"chapter-1"
		));
		assert!(!chapter_url_matches(
			"https://novelbuddy.me/shadow-slave",
			""
		));
	}

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
		let filters = vec![FilterValue::MultiSelect {
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
