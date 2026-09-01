#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
	imports::std::send_partial_result,
	prelude::*,
};
mod helpers;
mod models;
use helpers::{
	ContentType, body_to_text, content_type_from_setting, decode_manga_key, deep_link_manga_key,
	fetch_chapters, list_url, manga_from_detail, manga_page_result, request, valid_number,
	valid_slug,
};
use models::{ChapterBody, ListResponse, SeriesChapterBody};

pub const BASE_URL: &str = "https://chikari.moe";
pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1";

const SORTS: &[&str] = &[
	"popular",
	"trending",
	"top_rated",
	"updated",
	"added",
	"most_bookmarked",
];

const LIST_PAGE_SIZE: u32 = 60;
const SEARCH_PAGE_SIZE: u32 = 36;

struct Chikari;

impl Source for Chikari {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let limit = SEARCH_PAGE_SIZE;
		let offset = (page.max(1) as u32 - 1) * limit;
		let sort = filters
			.iter()
			.find_map(|filter| match filter {
				FilterValue::Sort { index, .. } => SORTS.get(*index as usize).copied(),
				_ => None,
			})
			.unwrap_or("popular");
		let content_type = content_type_from_setting();
		let data: ListResponse = request(&list_url(
			content_type,
			sort,
			limit,
			offset,
			query.as_deref(),
			&filters,
		))?;
		Ok(manga_page_result(data, content_type, offset, limit))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let (content_type, raw_slug) = decode_manga_key(&manga.key, manga.url.as_deref());
		let raw_slug = String::from(raw_slug);
		if needs_details {
			let url = format!("{BASE_URL}{}/{}", content_type.api_base(), raw_slug);
			let updated = manga_from_detail(request(&url)?, content_type);
			manga.copy_from(updated);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			manga.chapters = Some(fetch_chapters(&raw_slug, content_type)?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let (content_type, raw_slug) = decode_manga_key(&manga.key, manga.url.as_deref());
		match content_type {
			ContentType::Novel => {
				let data: ChapterBody = request(&format!(
					"{BASE_URL}{}/{}/chapters/{}/read",
					content_type.api_base(),
					raw_slug,
					chapter.key
				))?;
				let text = if data.locked || data.body.trim().is_empty() {
					"This chapter is locked (early access)".into()
				} else {
					body_to_text(data.body)?
				};
				Ok(vec![Page {
					content: PageContent::text(text),
					..Default::default()
				}])
			}
			ContentType::Series => {
				let data: SeriesChapterBody = request(&format!(
					"{BASE_URL}{}/{}/chapters/{}",
					content_type.api_base(),
					raw_slug,
					chapter.key
				))?;
				Ok(data
					.pages
					.into_iter()
					.map(|url| Page {
						content: PageContent::url(url),
						..Default::default()
					})
					.collect())
			}
		}
	}
}

impl ListingProvider for Chikari {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = SORTS
			.iter()
			.find(|s| **s == listing.id)
			.ok_or_else(|| error!("Unknown listing: {}", listing.id))?;
		let content_type = content_type_from_setting();
		let offset = (page.max(1) as u32 - 1) * LIST_PAGE_SIZE;
		let data: ListResponse = request(&list_url(
			content_type,
			sort,
			LIST_PAGE_SIZE,
			offset,
			None,
			&[],
		))?;
		Ok(manga_page_result(
			data,
			content_type,
			offset,
			LIST_PAGE_SIZE,
		))
	}
}

impl DeepLinkHandler for Chikari {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.split(['?', '#']).next().unwrap_or("");
		let Some(path) = path.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
		if parts.len() == 2
			&& (parts[0] == "novels" || parts[0] == "series")
			&& valid_slug(parts[1])
		{
			return Ok(Some(DeepLinkResult::Manga {
				key: deep_link_manga_key(ContentType::from_segment(parts[0]), parts[1]),
			}));
		}
		if parts.len() == 3
			&& (parts[0] == "novels" || parts[0] == "series")
			&& valid_slug(parts[1])
			&& valid_number(parts[2])
		{
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: deep_link_manga_key(ContentType::from_segment(parts[0]), parts[1]),
				key: parts[2].into(),
			}));
		}
		if parts.len() == 4
			&& (parts[0] == "novels" || parts[0] == "series")
			&& parts[2] == "chapters"
			&& valid_slug(parts[1])
			&& valid_number(parts[3])
		{
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: deep_link_manga_key(ContentType::from_segment(parts[0]), parts[1]),
				key: parts[3].into(),
			}));
		}
		Ok(None)
	}
}
register_source!(Chikari, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod tests {
	use super::*;
	use crate::helpers::chapter_key;
	use aidoku_test::aidoku_test;
	#[aidoku_test]
	fn search_returns_shadow_slave() {
		let result = Chikari
			.get_search_manga_list(Some("shadow slave".into()), 1, Vec::new())
			.expect("search failed");
		assert!(
			result
				.entries
				.iter()
				.any(|m| m.title.to_ascii_lowercase().contains("shadow slave"))
		);
	}

	#[aidoku_test]
	fn details_fetch_all_chapters() {
		let manga = Chikari
			.get_manga_update(
				Manga {
					key: "shadow-slave".into(),
					..Default::default()
				},
				true,
				true,
			)
			.expect("update failed");
		assert_eq!(manga.title, "Shadow Slave");
		assert!(manga.description.is_some());
		let chapters = manga.chapters.expect("chapters missing");
		assert!(chapters.len() > 500);
		assert!(
			chapters
				.iter()
				.any(|chapter| chapter.date_uploaded.is_some())
		);
		assert!(
			chapters.first().and_then(|chapter| chapter.chapter_number)
				> chapters.last().and_then(|chapter| chapter.chapter_number)
		);
	}

	#[aidoku_test]
	fn chapter_one_is_text() {
		let pages = Chikari
			.get_page_list(
				Manga {
					key: "shadow-slave".into(),
					..Default::default()
				},
				Chapter {
					key: chapter_key(1.0),
					..Default::default()
				},
			)
			.expect("chapter failed");
		assert_eq!(pages.len(), 1);
		match &pages[0].content {
			PageContent::Text(text) => assert!(!text.is_empty()),
			_ => panic!("expected text"),
		}
	}

	#[aidoku_test]
	fn series_deep_link_update_preserves_typed_key() {
		let manga = Chikari
			.get_manga_update(
				Manga {
					key: "series:alya-sometimes-hides-her-feelings-in-russian".into(),
					..Default::default()
				},
				true,
				false,
			)
			.expect("series update failed");
		assert_eq!(
			manga.key,
			"series:alya-sometimes-hides-her-feelings-in-russian"
		);
		assert_eq!(
			manga.url.as_deref(),
			Some("https://chikari.moe/series/alya-sometimes-hides-her-feelings-in-russian")
		);
	}

	#[aidoku_test]
	fn series_deep_link_fetches_image_pages_without_url() {
		let pages = Chikari
			.get_page_list(
				Manga {
					key: "series:alya-sometimes-hides-her-feelings-in-russian".into(),
					..Default::default()
				},
				Chapter {
					key: "86".into(),
					..Default::default()
				},
			)
			.expect("series chapter failed");
		assert!(!pages.is_empty());
		assert!(
			pages
				.iter()
				.any(|page| matches!(&page.content, PageContent::Url(_, _)))
		);
	}

	#[aidoku_test]
	fn deep_links_resolve() {
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/novels/shadow-slave".into()).unwrap(), Some(DeepLinkResult::Manga { key }) if key == "shadow-slave")
		);
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/series/shadow-slave".into()).unwrap(), Some(DeepLinkResult::Manga { key }) if key == "series:shadow-slave")
		);
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/novels/shadow-slave/1".into()).unwrap(), Some(DeepLinkResult::Chapter { manga_key, key }) if manga_key == "shadow-slave" && key == "1")
		);
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/series/shadow-slave/1".into()).unwrap(), Some(DeepLinkResult::Chapter { manga_key, key }) if manga_key == "series:shadow-slave" && key == "1")
		);
		assert!(
			Chikari
				.handle_deep_link("https://chikari.moe/series/shadow-slave/0".into())
				.unwrap()
				.is_none()
		);
	}
}
