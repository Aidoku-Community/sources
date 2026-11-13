#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
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
	ContentType, body_to_text, decode_manga_key, deep_link_manga_key, fetch_chapters, list_url,
	manga_from_detail, manga_page_result, request, valid_number, valid_slug,
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
	"bingeable",
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
				FilterValue::Sort { index, .. } => SORTS.get(*index as usize).copied(),
				_ => None,
			})
			.unwrap_or("popular");
		let hide = settings::hide_mtl() && !query.as_deref().is_some_and(mtl::mentions_mtl);
		let offset = (page.max(1) as u32 - 1) * limit;
		let Some(url) = list_url(sort, limit, offset, query.as_deref(), &filters) else {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		};
		let data: ListResponse = request(&url)?;
		let mut result = manga_page_result(data, offset, limit);
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
		let sort = SORTS
			.iter()
			.find(|s| **s == listing.id)
			.ok_or_else(|| error!("Unknown listing: {}", listing.id))?;
		let filters = apply_settings(Vec::new());
		let offset = (page.max(1) as u32 - 1) * LIST_PAGE_SIZE;
		let Some(url) = list_url(sort, LIST_PAGE_SIZE, offset, None, &filters) else {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		};
		let data: ListResponse = request(&url)?;
		let mut result = manga_page_result(data, offset, LIST_PAGE_SIZE);
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

register_source!(Chikari, ListingProvider, DeepLinkHandler);

fn apply_settings(filters: Vec<FilterValue>) -> Vec<FilterValue> {
	let hidden = settings::hidden_genres();
	let selected_types = settings::type_filters();
	let selected_ratings = settings::rating_filters();
	let selected_status = settings::status_filters();
	if hidden.is_empty()
		&& selected_types.is_empty()
		&& selected_ratings.is_empty()
		&& selected_status.is_empty()
	{
		return filters;
	}
	let mut filters = filters;
	if !hidden.is_empty() {
		if let Some(FilterValue::MultiSelect {
			included, excluded, ..
		}) = filters
			.iter_mut()
			.find(|filter| matches!(filter, FilterValue::MultiSelect { id, .. } if id == "genres"))
		{
			excluded.extend(helpers::hidden_genres_to_exclude(hidden, included));
		} else {
			filters.push(FilterValue::MultiSelect {
				id: "genres".into(),
				included: Vec::new(),
				excluded: hidden,
			});
		}
	}
	push_persistent_selection(&mut filters, "types", selected_types);
	push_persistent_selection(&mut filters, "ratings", selected_ratings);
	push_persistent_selection(&mut filters, "status", selected_status);
	filters
}

fn push_persistent_selection(
	filters: &mut Vec<FilterValue>,
	id: &str,
	selected: Vec<&'static str>,
) {
	if selected.is_empty() || has_filter(filters, id) {
		return;
	}
	filters.push(FilterValue::MultiSelect {
		id: id.into(),
		included: selected.into_iter().map(Into::into).collect(),
		excluded: Vec::new(),
	});
}

fn has_filter(filters: &[FilterValue], id: &str) -> bool {
	filters
		.iter()
		.any(|filter| matches!(filter, FilterValue::MultiSelect { id: found, .. } if found == id))
}

fn novel_text_pages(data: ChapterBody) -> Result<Vec<Page>> {
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

fn series_image_pages(pages: Vec<String>) -> Vec<Page> {
	pages
		.into_iter()
		.map(|url| Page {
			content: PageContent::url(url),
			..Default::default()
		})
		.collect()
}

fn chapter_deep_link(manga_key: String, number: &str) -> Option<DeepLinkResult> {
	let number: f32 = number.parse().ok()?;
	Some(DeepLinkResult::Chapter {
		manga_key,
		key: helpers::chapter_key(number),
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::helpers::chapter_key;
	use aidoku_test::aidoku_test;
	#[aidoku_test]
	fn search_finds_shadow_slave() {
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
	fn search_sees_every_content_rating() {
		let result = Chikari
			.get_search_manga_list(Some("city of witches".into()), 1, Vec::new())
			.expect("search failed");
		assert!(
			result.entries.iter().any(|m| m.key == "city-of-witches"),
			"expected the erotica-tier title in default results (update the slug if the catalog dropped it)"
		);
	}

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
	fn series_chapter_pages_are_images() {
		let mut manga = Chikari
			.get_manga_update(
				Manga {
					key: "series:alya-sometimes-hides-her-feelings-in-russian".into(),
					..Default::default()
				},
				false,
				true,
			)
			.expect("series update failed");
		let mut chapters = manga.chapters.take().expect("chapters missing");
		let chapter = chapters.pop().expect("no chapters");
		let pages = Chikari
			.get_page_list(manga, chapter)
			.expect("series chapter failed");
		assert!(!pages.is_empty());
		assert!(
			pages
				.iter()
				.all(|page| matches!(&page.content, PageContent::Url(_, _)))
		);
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

	#[aidoku_test]
	fn deep_links_normalize_chapter_numbers() {
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/novels/shadow-slave/1.50".into()).unwrap(), Some(DeepLinkResult::Chapter { manga_key, key }) if manga_key == "shadow-slave" && key == "1.5")
		);
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/series/shadow-slave/chapters/02".into()).unwrap(), Some(DeepLinkResult::Chapter { manga_key, key }) if manga_key == "series:shadow-slave" && key == "2")
		);
	}

	#[aidoku_test]
	fn deep_links_accept_host_and_trailing_slash_variants() {
		assert!(
			matches!(Chikari.handle_deep_link("https://www.chikari.moe/novels/shadow-slave/".into()).unwrap(), Some(DeepLinkResult::Manga { key }) if key == "shadow-slave")
		);
		assert!(
			matches!(Chikari.handle_deep_link("http://chikari.moe/series/shadow-slave/1/".into()).unwrap(), Some(DeepLinkResult::Chapter { manga_key, key }) if manga_key == "series:shadow-slave" && key == "1")
		);
		assert!(
			Chikari
				.handle_deep_link("https://example.com/novels/shadow-slave".into())
				.unwrap()
				.is_none()
		);
		assert!(
			Chikari
				.handle_deep_link("https://evil.com/chikari.moe/novels/shadow-slave".into())
				.unwrap()
				.is_none()
		);
		assert!(
			Chikari
				.handle_deep_link("https://chikari.moe.evil.com/novels/shadow-slave".into())
				.unwrap()
				.is_none()
		);
	}
}
