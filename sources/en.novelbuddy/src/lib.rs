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
mod models;

use helpers::{fetch_chapter_list, request, resolve_slug};
use models::{ChapterDetailData, ListData, TitleDetailData, TrendingData};

pub const BASE_URL: &str = "https://novelbuddy.com";
pub const API_URL: &str = "https://api.novelbuddy.com";
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

		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(s) = SORT_IDS.get(index as usize) {
						qs.push("sort", Some(*s));
						has_sort = true;
					}
				}
				FilterValue::Select { id, value } if id == "status" && value != "all" => {
					qs.push("status", Some(&value));
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genres" => {
					if !included.is_empty() {
						qs.push("genres", Some(&included.join(",")));
					}
					if !excluded.is_empty() {
						qs.push("exclude", Some(&excluded.join(",")));
					}
				}
				_ => {}
			}
		}

		if !has_sort {
			qs.push("sort", Some("popular"));
		}
		qs.push("page", Some(&page.to_string()));
		if let Some(q) = query.as_deref() {
			qs.push("q", Some(q));
		}

		let url = format!("{API_URL}/titles/search?{qs}");
		let data: ListData = request(&url)?;
		Ok(MangaPageResult {
			entries: data.items.into_iter().map(Manga::from).collect(),
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
			manga.chapters = Some(fetch_chapter_list(&manga.key)?);
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
			.map(helpers::html_to_text)
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
				Ok(MangaPageResult {
					entries: data.items.into_iter().map(Manga::from).collect(),
					has_next_page: false,
				})
			}
			"latest" => {
				let url = format!("{API_URL}/titles/search?sort=latest&page={page}");
				let data: ListData = request(&url)?;
				Ok(MangaPageResult {
					entries: data.items.into_iter().map(Manga::from).collect(),
					has_next_page: data.pagination.has_next,
				})
			}
			_ => bail!("Unknown listing: {}", listing.id),
		}
	}
}

impl DeepLinkHandler for NovelBuddy {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split(['?', '#'])
			.next()
			.unwrap_or(&url)
			.rsplit("novelbuddy.com")
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
	}

	#[aidoku_test]
	fn page_list_returns_text_page() {
		let source = NovelBuddy;
		let manga = Manga {
			key: "VYPGVZ8z".into(),
			..Default::default()
		};
		let chapter = Chapter {
			key: "2ZejwbQD".into(),
			..Default::default()
		};
		let pages = source
			.get_page_list(manga, chapter)
			.expect("get_page_list failed");
		assert_eq!(pages.len(), 1);
		match &pages[0].content {
			PageContent::Text(text) => {
				assert!(!text.is_empty());
				assert!(
					text.to_lowercase().contains("sunny"),
					"expected chapter text to mention 'Sunny'"
				);
			}
			_ => panic!("expected PageContent::Text"),
		}
	}

	#[aidoku_test]
	fn deep_link_resolves_series() {
		let source = NovelBuddy;
		let result = source
			.handle_deep_link("https://novelbuddy.com/shadow-slave".into())
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Manga { key } => assert_eq!(key, "VYPGVZ8z"),
			_ => panic!("expected Manga deep link"),
		}
	}
}
