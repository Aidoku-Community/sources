#![no_std]
mod api;
mod helpers;
mod home;
mod models;
mod settings;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec, vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};
use api::{API_URL, BASE_URL, SORTS, SearchParams, get, parse, site_headers};
use helpers::{Link, parse_link, site_languages};
use models::{ApiResponse, ChapterDetail, ChapterItem, Title, chapter_list};

struct MangaBall;

impl Source for MangaBall {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(Link::Title(id)) = query.as_deref().and_then(parse_link)
			&& let Ok(found) = get::<ApiResponse<Title>>(&format!("{API_URL}/title/detail/{id}"))
		{
			return Ok(MangaPageResult {
				entries: vec![Manga::try_from(found.data)?],
				has_next_page: false,
			});
		}

		let mut search = SearchParams::new(query);
		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					if let Some(sort) = SORTS.get(index as usize) {
						search.sort = sort;
					}
					search.ascending = ascending;
				}
				FilterValue::Select { id, value } if !value.is_empty() => match id.as_str() {
					"type" => search.kind = if value == "any" { String::new() } else { value },
					"status" => search.status = value,
					"demographic" => search.demographic = value,
					"rating" => search.adult_mode = value,
					"tag_mode" => search.tag_mode = value,
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"genre" | "theme" | "format" | "content" => {
						search.included_tags.extend(included);
						search.excluded_tags.extend(excluded);
					}
					_ => {}
				},
				_ => {}
			}
		}
		// A tag included in a search overrides its exclusion in settings.
		let included = &search.included_tags;
		search.excluded_tags.retain(|id| !included.contains(id));

		search.page(page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{API_URL}/title/detail/{}", manga.key);
			let details = get::<ApiResponse<Title>>(&url)?.data.into_details()?;
			manga.copy_from(details);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let languages = site_languages(settings::languages());
			let excluded = settings::excluded_scanlators();
			let url = format!("{API_URL}/chapter/chapter-listing-by-title-id");
			// The API rejects a content type that carries a charset.
			let request = site_headers(Request::post(url)?)
				.header("Content-Type", "application/json")
				.body(format!(r#"{{"title_id":"{}"}}"#, manga.key));
			let chapters = parse::<ApiResponse<Vec<ChapterItem>>>(request.send()?)?.data;
			manga.chapters = Some(chapter_list(
				chapters,
				&languages,
				&excluded,
				settings::deduplicate_chapters(),
			));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{API_URL}/chapter-detail?chapter_id={}", chapter.key);
		let pages = get::<ApiResponse<ChapterDetail>>(&url)?.data.chapter.pages;
		if pages.is_empty() || pages.iter().any(|url| url.trim().is_empty()) {
			bail!("This chapter has missing pages");
		}
		Ok(pages
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for MangaBall {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let Some(sort) = SORTS.iter().find(|sort| **sort == listing.id) else {
			bail!("Unknown listing");
		};
		SearchParams::sorted(sort).page(page)
	}
}

impl ImageRequestProvider for MangaBall {
	// The image hosts refuse requests the site didn't refer.
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for MangaBall {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(match parse_link(&url) {
			Some(Link::Title(key)) => Some(DeepLinkResult::Manga { key }),
			Some(Link::Chapter(key)) => {
				let url = format!("{API_URL}/chapter-detail?chapter_id={key}");
				get::<ApiResponse<ChapterDetail>>(&url)?
					.data
					.chapter
					.title_id
					.map(|manga_key| DeepLinkResult::Chapter { manga_key, key })
			}
			None => None,
		})
	}
}

register_source!(
	MangaBall,
	Home,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
