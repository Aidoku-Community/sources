#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Manga, MangaPageResult, Page,
	PageContent, Result, Source,
	alloc::{String, Vec, vec},
	helpers::uri::QueryParameters,
	imports::std::send_partial_result,
	prelude::*,
};

mod helpers;

use helpers::{
	build_chapter_url, build_novel_url, content_rating_from_tags, extract_authors,
	extract_chapter_text, extract_chapters, extract_cover, extract_description, extract_tags,
	extract_title, parse_novel_and_chapter, parse_search_results, request_html,
};

pub const BASE_URL: &str = "https://freewebnovel.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
	(KHTML, like Gecko) Version/17.0 Safari/605.1.15";

struct FreeWebNovel;

impl Source for FreeWebNovel {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let Some(query) = query else {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		};
		if page > 1 {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		}
		let mut qs = QueryParameters::new();
		qs.push("searchkey", Some(&query));
		let url = format!("{BASE_URL}/search?{qs}");
		let html = request_html(&url)?;
		let entries = parse_search_results(&html);
		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = build_novel_url(&manga.key);
		let html = request_html(&url)?;

		if needs_details {
			manga.title = extract_title(&html).unwrap_or(manga.title);
			manga.cover = extract_cover(&html);
			manga.description = extract_description(&html);
			manga.authors = extract_authors(&html);
			manga.tags = extract_tags(&html);
			if let Some(tags) = manga.tags.as_deref() {
				manga.content_rating = content_rating_from_tags(tags);
			}
			manga.url = Some(url.clone());
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let chapters = extract_chapters(&html, &manga.key);
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = build_chapter_url(&manga.key, &chapter.key);
		let html = request_html(&url)?;
		let text = extract_chapter_text(&html);
		let text = if text.is_empty() {
			"(empty chapter)".into()
		} else {
			text
		};
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl DeepLinkHandler for FreeWebNovel {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some((slug, chapter_key)) = parse_novel_and_chapter(&url) else {
			return Ok(None);
		};
		if let Some(key) = chapter_key {
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: slug,
				key,
			}))
		} else {
			Ok(Some(DeepLinkResult::Manga { key: slug }))
		}
	}
}

register_source!(FreeWebNovel, DeepLinkHandler);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn search_returns_results() {
		let source = FreeWebNovel;
		let result = source
			.get_search_manga_list(Some("shadow slave".into()), 1, Vec::new())
			.expect("search failed");
		assert!(!result.entries.is_empty(), "expected at least one result");
		assert!(
			result
				.entries
				.iter()
				.any(|m| m.title.to_ascii_lowercase().contains("shadow slave")),
			"expected 'Shadow Slave' in results"
		);
	}

	#[aidoku_test]
	fn series_detail_has_many_chapters() {
		let source = FreeWebNovel;
		let manga = Manga {
			key: "swordmasters-youngest-son-novel".into(),
			..Default::default()
		};
		let manga = source
			.get_manga_update(manga, true, true)
			.expect("get_manga_update failed");
		assert!(manga.title.to_ascii_lowercase().contains("swordmaster"));
		let chapters = manga.chapters.expect("no chapters returned");
		assert!(
			chapters.len() > 50,
			"expected lots of chapters, got {}",
			chapters.len()
		);
	}

	#[aidoku_test]
	fn page_list_returns_text_page() {
		let source = FreeWebNovel;
		let manga = Manga {
			key: "swordmasters-youngest-son-novel".into(),
			..Default::default()
		};
		let chapter = Chapter {
			key: "chapter-1".into(),
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
					text.len() > 50,
					"expected chapter text to be substantial"
				);
			}
			_ => panic!("expected PageContent::Text"),
		}
	}

	#[aidoku_test]
	fn deep_link_resolves_chapter() {
		let source = FreeWebNovel;
		let result = source
			.handle_deep_link(
				"https://freewebnovel.com/novel/swordmasters-youngest-son-novel/chapter-1"
					.into(),
			)
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Chapter { manga_key, key } => {
				assert_eq!(manga_key, "swordmasters-youngest-son-novel");
				assert_eq!(key, "chapter-1");
			}
			_ => panic!("expected Chapter deep link"),
		}
	}
}
