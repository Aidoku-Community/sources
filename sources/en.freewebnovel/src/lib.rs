#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeComponentValue, HomeLayout, Link, Listing, ListingProvider, Manga, MangaPageResult,
	Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
	helpers::uri::QueryParameters,
	imports::std::send_partial_result,
	prelude::*,
};

mod helpers;

use helpers::{
	build_chapter_url, build_novel_url, content_rating_from_tags, extract_authors,
	extract_chapter_text, extract_chapters, extract_cover, extract_description, extract_tags,
	extract_title, parse_home_section, parse_novel_and_chapter, parse_search_results,
	request_html,
};

pub const BASE_URL: &str = "https://freewebnovel.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
	(KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const LISTING_LATEST_RELEASE: &str = "latest-release";
const LISTING_LATEST_NOVEL: &str = "latest-novel";

struct FreeWebNovel;

fn build_sort_url(kind: &str, page: i32) -> String {
	if page <= 1 {
		format!("{BASE_URL}/sort/{kind}")
	} else {
		format!("{BASE_URL}/sort/{kind}/{page}")
	}
}

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
			manga.title = extract_title(&html)?;
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
		let text = extract_chapter_text(&html)?;
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl Home for FreeWebNovel {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = request_html(BASE_URL)?;

		let latest_release = parse_home_section(&html, "LATEST RELEASE NOVELS");
		let latest_novels = parse_home_section(&html, "LATEST NOVELS");
		let mut hot_entries = parse_search_results(&html);
		if !hot_entries.is_empty() {
			let mut seen = Vec::new();
			for entry in latest_release.iter().chain(latest_novels.iter()) {
				if !seen.iter().any(|s| s == &entry.key) {
					seen.push(entry.key.clone());
				}
			}
			hot_entries.retain(|m| !seen.iter().any(|s| s == &m.key));
			hot_entries.truncate(12);
		}

		let mut components = Vec::new();
		if !hot_entries.is_empty() {
			components.push(HomeComponent {
				title: Some("Hot Novels".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: hot_entries.into_iter().map(Into::into).collect::<Vec<Link>>(),
					listing: None,
				},
				..Default::default()
			});
		}
		if !latest_release.is_empty() {
			components.push(HomeComponent {
				title: Some("Latest Release Novels".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: latest_release
						.into_iter()
						.map(Into::into)
						.collect::<Vec<Link>>(),
					listing: Some(Listing {
						id: LISTING_LATEST_RELEASE.into(),
						name: "Latest Release Novels".into(),
						..Default::default()
					}),
				},
				..Default::default()
			});
		}
		if !latest_novels.is_empty() {
			components.push(HomeComponent {
				title: Some("Latest Novels".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: latest_novels
						.into_iter()
						.map(Into::into)
						.collect::<Vec<Link>>(),
					listing: Some(Listing {
						id: LISTING_LATEST_NOVEL.into(),
						name: "Latest Novels".into(),
						..Default::default()
					}),
				},
				..Default::default()
			});
		}

		Ok(HomeLayout { components })
	}
}

impl ListingProvider for FreeWebNovel {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort_key = match listing.id.as_str() {
			LISTING_LATEST_RELEASE => "latest-release",
			LISTING_LATEST_NOVEL => "latest-novel",
			_ => {
				return Ok(MangaPageResult {
					entries: Vec::new(),
					has_next_page: false,
				});
			}
		};
		let url = build_sort_url(sort_key, page);
		let html = request_html(&url)?;
		let entries = parse_search_results(&html);
		let has_next_page = html
			.select_first(
				"a[rel='next'], a:contains(Next), li:contains(Next)",
			)
			.is_some()
			|| entries.len() >= 20;
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
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

register_source!(FreeWebNovel, Home, ListingProvider, DeepLinkHandler);

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
	fn chapters_include_first_chapter() {
		let source = FreeWebNovel;
		let manga = Manga {
			key: "swordmasters-youngest-son-novel".into(),
			..Default::default()
		};
		let manga = source
			.get_manga_update(manga, false, true)
			.expect("get_manga_update failed");
		let chapters = manga.chapters.expect("no chapters returned");
		assert!(
			chapters.iter().any(|c| c.key == "chapter-1"),
			"expected chapter-1 to be present"
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

	#[aidoku_test]
	fn deep_link_resolves_series() {
		let source = FreeWebNovel;
		let result = source
			.handle_deep_link(
				"https://freewebnovel.com/novel/swordmasters-youngest-son-novel".into(),
			)
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Manga { key } => {
				assert_eq!(key, "swordmasters-youngest-son-novel");
			}
			_ => panic!("expected Manga deep link"),
		}
	}
}
