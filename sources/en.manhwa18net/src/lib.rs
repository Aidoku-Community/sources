#![no_std]
mod helpers;
mod models;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		html::Html,
		net::{Request, TimeUnit, set_rate_limit},
	},
	prelude::*,
};
use helpers::{BASE_URL, absolute_url, fetch_list, fetch_props, parse_link};
use models::{ChapterDto, MangaDto, prop};

const REFERER: &str = "https://manhwa18.net/";

// Must match the order of the options in filters.json.
const SORTS: [&str; 4] = ["update", "top", "like", "new"];

struct Manhwa18Net;

impl Source for Manhwa18Net {
	fn new() -> Self {
		set_rate_limit(3, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut sort = SORTS[0];
		let mut statuses = Vec::new();
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					sort = usize::try_from(index)
						.ok()
						.and_then(|index| SORTS.get(index))
						.copied()
						.unwrap_or(SORTS[0]);
				}
				FilterValue::MultiSelect { id, included, .. } if id == "status" => {
					statuses = included;
				}
				_ => {}
			}
		}

		let mut params = QueryParameters::new();
		let path = match query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
			Some(query) => {
				params.push("q", Some(query));
				"tim-kiem"
			}
			None => {
				// the filters aren't supported by the search page
				params.push("sort", Some(sort));
				for status in &statuses {
					params.push(status, Some("1"));
				}
				"manga-list"
			}
		};
		params.push("page", Some(&page.to_string()));

		fetch_list(&format!("{BASE_URL}/{path}?{params}"))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let props = fetch_props(&format!("{BASE_URL}/manga/{}", manga.key))?;

		if needs_details {
			let Some(details) = prop::<MangaDto>(&props, "manga") else {
				bail!("Manga details not found");
			};
			// the key identifies the entry in the user's library, so it must not change
			let key = core::mem::take(&mut manga.key);
			manga.copy_from(details.into_details());
			manga.key = key;
		}

		if needs_chapters {
			let Some(chapters) = prop::<Vec<ChapterDto>>(&props, "chapters") else {
				bail!("Chapters not found");
			};
			manga.chapters = Some(
				chapters
					.into_iter()
					.map(|chapter| chapter.into_chapter(&manga.key))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let props = fetch_props(&format!("{BASE_URL}/manga/{}/{}", manga.key, chapter.key))?;
		let Some(content) = prop::<String>(&props, "chapterContent") else {
			bail!("Chapter content not found");
		};

		let content = Html::parse_fragment(content)?;
		let pages: Vec<Page> = content
			.select("img")
			.into_iter()
			.flatten()
			.filter_map(|img| {
				// lazy loaded images keep a placeholder in `src`
				["data-src", "data-lazy-src", "src"]
					.into_iter()
					.find_map(|attr| absolute_url(&img.attr(attr)?))
			})
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect();

		if pages.is_empty() {
			bail!("This chapter has no pages");
		}
		Ok(pages)
	}
}

impl ListingProvider for Manhwa18Net {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => "top",
			"liked" => "like",
			"new" => "new",
			_ => "update",
		};
		fetch_list(&format!("{BASE_URL}/manga-list?sort={sort}&page={page}"))
	}
}

impl ImageRequestProvider for Manhwa18Net {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", REFERER))
	}
}

impl DeepLinkHandler for Manhwa18Net {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(parse_link(&url).map(|(manga_key, chapter)| match chapter {
			Some(key) => DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: key.into(),
			},
			None => DeepLinkResult::Manga {
				key: manga_key.into(),
			},
		}))
	}
}

register_source!(
	Manhwa18Net,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
