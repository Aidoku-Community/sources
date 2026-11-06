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
mod settings;
mod watermark;

use helpers::{build_chapters, query_param, request};
use models::{ChapterDetailData, ListData, ListingData, NovelDetailData};

pub const BASE_URL: &str = "https://novelarchive.st";
pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 \
	 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1";

const SORT_IDS: &[&str] = &["recent", "popular", "chapters", "rating"];
const PAGE_SIZE: &str = "24";

struct NovelArchive;

impl Source for NovelArchive {
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
		let mut sort = "recent";
		let mut status: Option<String> = None;

		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(value) = SORT_IDS.get(index as usize) {
						sort = value;
					}
				}
				FilterValue::Select { id, value } if id == "status" => status = Some(value),
				_ => {}
			}
		}

		let status = status
			.filter(|value| value != "all")
			.unwrap_or_else(settings::status);

		qs.push("sort", Some(sort));
		qs.push("status", Some(&status));
		qs.push("limit", Some(PAGE_SIZE));
		qs.push("page", Some(&page.to_string()));

		if let Some(query) = query.as_deref() {
			qs.push("search", Some(query));
		}

		let url = format!("{BASE_URL}/api/novels?{qs}");
		let data: ListData = request(&url)?;
		Ok(MangaPageResult {
			entries: data.novels.into_iter().map(Manga::from).collect(),
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
			let url = format!("{BASE_URL}/api/novels/{}", manga.key);
			let data: NovelDetailData = request(&url)?;
			let mut novel = data.novel;
			let chapters = if needs_chapters {
				Some(build_chapters(
					&novel.id,
					core::mem::take(&mut novel.chapter_names),
				))
			} else {
				None
			};
			manga.copy_from(novel.into());
			if needs_chapters {
				send_partial_result(&manga);
				manga.chapters = chapters;
			}
		} else if needs_chapters {
			let url = format!("{BASE_URL}/api/novels/{}", manga.key);
			let data: NovelDetailData = request(&url)?;
			manga.chapters = Some(build_chapters(&data.novel.id, data.novel.chapter_names));
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{BASE_URL}/api/novels/{}/chapters/{}",
			manga.key, chapter.key
		);
		let data: ChapterDetailData = request(&url)?;
		let text = markdown::to_markdown(data.chapter.content.as_deref().unwrap_or_default());
		let text = if text.is_empty() {
			"(empty chapter)".to_string()
		} else {
			text
		};
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl ListingProvider for NovelArchive {
	fn get_manga_list(&self, listing: Listing, _page: i32) -> Result<MangaPageResult> {
		let path = match listing.id.as_str() {
			"trending" => "trending",
			"recent" => "recent",
			"recently-updated" => "recently-updated",
			_ => bail!("Unknown listing: {}", listing.id),
		};
		let url = format!("{BASE_URL}/api/novels/{path}");
		let data: ListingData = request(&url)?;
		Ok(MangaPageResult {
			entries: data.novels.into_iter().map(Manga::from).collect(),
			has_next_page: false,
		})
	}
}

impl DeepLinkHandler for NovelArchive {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some((base, query)) = url.split_once('?') else {
			return Ok(None);
		};
		let path = [
			"https://novelarchive.st/",
			"http://novelarchive.st/",
			"https://www.novelarchive.st/",
			"http://www.novelarchive.st/",
		]
		.iter()
		.find_map(|prefix| base.strip_prefix(prefix))
		.map(|path| path.trim_end_matches('/'))
		.unwrap_or_default();
		let query = query.split('#').next().unwrap_or_default();

		match path {
			"novel" => {
				let Some(id) = query_param(query, "id") else {
					return Ok(None);
				};
				if id.is_empty() {
					return Ok(None);
				}
				Ok(Some(DeepLinkResult::Manga { key: id.into() }))
			}
			"reader" => {
				let (Some(id), Some(chapter)) =
					(query_param(query, "novel"), query_param(query, "chapter"))
				else {
					return Ok(None);
				};
				if id.is_empty() || chapter.is_empty() {
					return Ok(None);
				}
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: id.into(),
					key: chapter.into(),
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(NovelArchive, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	const SHADOW_SLAVE_KEY: &str = "69faa859a5f4c7d1b734d496";

	#[aidoku_test]
	fn search_returns_results() {
		let source = NovelArchive;
		let result = source
			.get_search_manga_list(Some("shadow slave".into()), 1, Vec::new())
			.expect("search failed");
		assert!(!result.entries.is_empty(), "expected at least one result");
		assert!(
			result
				.entries
				.iter()
				.any(|manga| manga.title.to_lowercase().contains("shadow slave")),
			"expected 'Shadow Slave' in results"
		);
	}

	#[aidoku_test]
	fn series_detail_has_all_chapters() {
		let source = NovelArchive;
		let manga = Manga {
			key: SHADOW_SLAVE_KEY.into(),
			..Default::default()
		};
		let manga = source
			.get_manga_update(manga, true, true)
			.expect("get_manga_update failed");
		assert_eq!(manga.title, "Shadow Slave");
		assert!(manga.description.is_some());
		assert!(manga.authors.is_some());
		assert!(matches!(manga.status, aidoku::MangaStatus::Ongoing));
		let chapters = manga.chapters.expect("no chapters returned");
		assert_eq!(chapters.len(), 3234);
		assert_eq!(chapters.first().map(|c| c.key.as_str()), Some("1"));
		assert_eq!(chapters.last().map(|c| c.key.as_str()), Some("3234"));
	}

	#[aidoku_test]
	fn page_list_returns_text_page() {
		let source = NovelArchive;
		let manga = Manga {
			key: SHADOW_SLAVE_KEY.into(),
			..Default::default()
		};
		let chapter = Chapter {
			key: "1".into(),
			..Default::default()
		};
		let pages = source
			.get_page_list(manga, chapter)
			.expect("get_page_list failed");
		assert_eq!(pages.len(), 1);
		match &pages[0].content {
			PageContent::Text(text) => {
				assert_ne!(text, "(empty chapter)", "chapter body was empty");
				assert!(text.len() > 500, "expected a full chapter body");
				assert!(
					text.contains("\n\n***\n\n"),
					"scene break should stay markdown, not be escaped"
				);
				assert!(
					!text.contains("\\*"),
					"emphasis markers should not be escaped"
				);
			}
			_ => panic!("expected PageContent::Text"),
		}
	}

	#[aidoku_test]
	fn deep_link_ignores_foreign_hosts() {
		let source = NovelArchive;
		let result = source
			.handle_deep_link(format!(
				"https://notnovelarchive.st/novel?id={SHADOW_SLAVE_KEY}"
			))
			.expect("deep link failed");
		assert!(result.is_none(), "expected no deep link for a foreign host");
	}

	#[aidoku_test]
	fn deep_link_resolves_series() {
		let source = NovelArchive;
		let result = source
			.handle_deep_link(format!(
				"https://novelarchive.st/novel?id={SHADOW_SLAVE_KEY}"
			))
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Manga { key } => assert_eq!(key, SHADOW_SLAVE_KEY),
			_ => panic!("expected Manga deep link"),
		}
	}

	#[aidoku_test]
	fn deep_link_resolves_chapter() {
		let source = NovelArchive;
		let result = source
			.handle_deep_link(format!(
				"https://novelarchive.st/reader?novel={SHADOW_SLAVE_KEY}&chapter=1"
			))
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Chapter { manga_key, key } => {
				assert_eq!(manga_key, SHADOW_SLAVE_KEY);
				assert_eq!(key, "1");
			}
			_ => panic!("expected Chapter deep link"),
		}
	}
}
