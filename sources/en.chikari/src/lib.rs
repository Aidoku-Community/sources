#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, Listing,
	ListingProvider, Manga, MangaPageResult, MultiSelectFilter, Page, Result, Source,
	alloc::{String, Vec, borrow::Cow, vec},
	imports::std::send_partial_result,
	prelude::*,
};

mod helpers;
mod markdown;
mod models;
mod mtl;
mod settings;
mod watermark;

use helpers::{
	ContentType, apply_settings, chapter_deep_link, decode_manga_key, deep_link_manga_key,
	fetch_chapters, list_url, manga_from_detail, manga_page_result, novel_text_pages,
	pinned_rating, request, selected_ratings, series_image_pages, valid_number, valid_slug,
};
use models::{ChapterBody, ListResponse, SeriesChapterBody};

pub const BASE_URL: &str = "https://chikari.moe";
pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1";

/// Search resolves a sort filter by its position here, so this order must match
/// the sort ids in res/filters.json.
const SEARCH_SORTS: &[&str] = &[
	"popular",
	"top_rated",
	"bingeable",
	"gems",
	"trending",
	"updated",
	"added",
	"most_bookmarked",
	"random",
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
		let filters = apply_settings(filters);
		let limit = SEARCH_PAGE_SIZE;
		let sort = filters
			.iter()
			.find_map(|filter| match filter {
				FilterValue::Sort { index, .. } => SEARCH_SORTS.get(*index as usize).copied(),
				_ => None,
			})
			.unwrap_or("popular");
		let hide = settings::hide_mtl() && !query.as_deref().is_some_and(mtl::mentions_mtl);
		let offset = (page.max(1) as u32 - 1) * limit;
		let Some(url) = list_url(sort, limit, offset, query.as_deref(), &filters) else {
			return Ok(MangaPageResult::default());
		};
		let data: ListResponse = request(&url)?;
		let mut result = manga_page_result(
			data,
			offset,
			limit,
			pinned_rating(&selected_ratings(&filters)),
		);
		if hide {
			mtl::retain_manga(&mut result.entries);
		}
		Ok(result)
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
			let mut chapters = fetch_chapters(&raw_slug, content_type)?;
			if settings::hide_mtl() {
				mtl::retain_chapters(&mut chapters);
			}
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let (content_type, raw_slug) = decode_manga_key(&manga.key, manga.url.as_deref());
		match content_type {
			ContentType::Novel => {
				let data: ChapterBody = request(&format!(
					"{BASE_URL}{}/{raw_slug}/chapters/{}/read",
					content_type.api_base(),
					chapter.key
				))?;
				novel_text_pages(data)
			}
			ContentType::Series => {
				let data: SeriesChapterBody = request(&format!(
					"{BASE_URL}{}/{raw_slug}/chapters/{}",
					content_type.api_base(),
					chapter.key
				))?;
				Ok(series_image_pages(data.pages))
			}
		}
	}
}

impl ListingProvider for Chikari {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = SEARCH_SORTS
			.iter()
			.find(|s| **s == listing.id)
			.ok_or_else(|| error!("Unknown listing: {}", listing.id))?;
		let filters = apply_settings(Vec::new());
		let offset = (page.max(1) as u32 - 1) * LIST_PAGE_SIZE;
		let Some(url) = list_url(sort, LIST_PAGE_SIZE, offset, None, &filters) else {
			return Ok(MangaPageResult::default());
		};
		let data: ListResponse = request(&url)?;
		let mut result = manga_page_result(
			data,
			offset,
			LIST_PAGE_SIZE,
			pinned_rating(&selected_ratings(&filters)),
		);
		if settings::hide_mtl() {
			mtl::retain_manga(&mut result.entries);
		}
		Ok(result)
	}
}

impl DeepLinkHandler for Chikari {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = helpers::chikari_path(&url) else {
			return Ok(None);
		};
		let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
		let (kind, slug, rest) = match parts.as_slice() {
			[kind @ ("novels" | "series"), slug, rest @ ..] if valid_slug(slug) => {
				(*kind, *slug, rest)
			}
			_ => return Ok(None),
		};
		let manga_key = deep_link_manga_key(ContentType::from_segment(kind), slug);
		Ok(match rest {
			[] => Some(DeepLinkResult::Manga { key: manga_key }),
			[number] if valid_number(number) => chapter_deep_link(manga_key, number),
			["chapters", number] if valid_number(number) => chapter_deep_link(manga_key, number),
			_ => None,
		})
	}
}

impl DynamicFilters for Chikari {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let genres = helpers::genres()?;
		let (options, ids): (Vec<Cow<'static, str>>, Vec<Cow<'static, str>>) = genres
			.into_iter()
			.map(|genre| (genre.name.into(), genre.slug.into()))
			.unzip();
		Ok(vec![
			MultiSelectFilter {
				id: "genres".into(),
				title: Some("Genres".into()),
				is_genre: true,
				can_exclude: true,
				uses_tag_style: true,
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		])
	}
}

register_source!(Chikari, ListingProvider, DeepLinkHandler, DynamicFilters);

#[cfg(test)]
mod tests {
	use super::*;
	use crate::helpers::chapter_key;
	use aidoku::PageContent;
	use aidoku_test::aidoku_test;
	#[aidoku_test]
	fn novel_update_fetches_details_and_chapters() {
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
	fn novel_chapter_pages_are_text() {
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
			.expect("novel chapter failed");
		assert!(matches!(&pages[0].content, PageContent::Text(text) if !text.is_empty()));
	}

	#[aidoku_test]
	fn locked_and_empty_chapters_show_the_early_access_message() {
		for data in [
			ChapterBody {
				body: "prose".into(),
				locked: true,
			},
			ChapterBody {
				body: " ".into(),
				locked: false,
			},
		] {
			let pages = novel_text_pages(data).expect("novel chapter failed");
			assert!(
				matches!(&pages[0].content, PageContent::Text(text) if text == "This chapter is locked (early access)")
			);
		}
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
			Chikari
				.handle_deep_link("https://chikari.moe/series/shadow-slave/0".into())
				.unwrap()
				.is_none()
		);
	}
}
