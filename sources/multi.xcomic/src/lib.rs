#![no_std]
extern crate alloc;

mod filters;
mod graphql;
mod helpers;
mod home;
mod models;
mod settings;

use aidoku::{
	BaseUrlProvider, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, Result,
	Source,
	alloc::{String, Vec, vec},
	imports::{defaults::defaults_get, net::Request, std::send_partial_result},
	prelude::*,
};
use graphql::BrowseParams;
use helpers::{chapter_from_data, manga_from_data};

const DEFAULT_BASE_URL: &str = "https://xcomic.me";

struct XComic;

impl XComic {
	fn browse_page(&self, base_url: &str, params: BrowseParams) -> Result<MangaPageResult> {
		let response = graphql::browse_request(base_url, &params)?.send()?;
		let (comics, has_next_page) = graphql::parse_browse(response, &params)?;
		let entries = comics
			.into_iter()
			.map(|comic| manga_from_data(comic, base_url))
			.collect();
		Ok(MangaPageResult {
			has_next_page,
			entries,
		})
	}
}

impl Source for XComic {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base_url = self.get_base_url()?;
		if let Some(target) = query.as_deref().and_then(helpers::target_from_query) {
			let key = helpers::comic_key(&base_url, target)?;
			let comic = graphql::fetch_comic(&base_url, &key)?;
			return Ok(MangaPageResult {
				entries: vec![manga_from_data(comic, &base_url)],
				has_next_page: false,
			});
		}
		let mut params = BrowseParams::new("field_score", page, graphql::PAGE_SIZE);
		params.word = query.unwrap_or_default();

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } if id == "sort" => {
					if let Some(sort) = graphql::SORT_IDS.get(index as usize) {
						params.sortby = (*sort).into();
					}
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"original_status" => params.original_status = value,
					"include_mode" => params.include_mode = value,
					"exclude_mode" => params.exclude_mode = value,
					"chapter_count" => params.chapter_count = value,
					_ => {}
				},
				FilterValue::Text { id, value } => match id.as_str() {
					"chapter_count_custom" if !value.trim().is_empty() => {
						params.chapter_count = helpers::chapter_count_range(&value);
					}
					"year" => {
						(params.year_min, params.year_max) = helpers::parse_range(&value);
					}
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"genres" => {
						params.included_genres = included;
						params.excluded_genres.extend(excluded);
					}
					"demographics" => params.demographics = included,
					"original_languages" => params.original_languages = included,
					// An empty selection means the reader narrowed nothing, so the
					// settings default this carries has to survive it.
					"types" if !included.is_empty() => params.types = included,
					_ => {}
				},
				_ => {}
			}
		}

		self.browse_page(&base_url, params)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let base_url = self.get_base_url()?;
		let (manga, team) = if needs_details {
			let comic = graphql::fetch_comic(&base_url, &manga.key)?;
			let team = helpers::team_of(&comic);
			let manga = Manga {
				chapters: manga.chapters,
				..manga_from_data(comic, &base_url)
			};
			if needs_chapters {
				send_partial_result(&manga);
			}
			(manga, team)
		} else {
			(manga, None)
		};
		Ok(if needs_chapters {
			Manga {
				chapters: Some(
					graphql::fetch_chapters(&base_url, &manga.key)?
						.into_iter()
						.filter_map(|chapter| {
							chapter_from_data(chapter, &base_url, None, team.as_deref(), false)
						})
						.collect(),
				),
				..manga
			}
		} else {
			manga
		})
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let base_url = self.get_base_url()?;
		let pages: Vec<Page> = graphql::fetch_page_urls(&base_url, &chapter.key)?
			.into_iter()
			.map(|url| helpers::absolute_url(&base_url, &url))
			.filter(|url| !url.is_empty())
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect();
		if pages.is_empty() {
			bail!("No pages found for this chapter");
		}
		Ok(pages)
	}
}

impl ListingProvider for XComic {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let base_url = self.get_base_url()?;
		self.browse_page(
			&base_url,
			BrowseParams::new(&listing.id, page, graphql::PAGE_SIZE),
		)
	}
}

impl ImageRequestProvider for XComic {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let base_url = self.get_base_url()?;
		Ok(Request::get(url)?
			.header("Referer", &format!("{base_url}/"))
			.header("Origin", &base_url))
	}
}

impl DeepLinkHandler for XComic {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(target) = helpers::parse_link(&url) else {
			return Ok(None);
		};
		Ok(Some(match target {
			helpers::Target::Comic(manga_key, Some(key)) => {
				DeepLinkResult::Chapter { manga_key, key }
			}
			target => DeepLinkResult::Manga {
				key: helpers::comic_key(&self.get_base_url()?, target)?,
			},
		}))
	}
}

impl BaseUrlProvider for XComic {
	fn get_base_url(&self) -> Result<String> {
		Ok(defaults_get::<String>("url")
			.filter(|url| !url.is_empty())
			.unwrap_or_else(|| DEFAULT_BASE_URL.into()))
	}
}

register_source!(
	XComic,
	Home,
	ListingProvider,
	DynamicFilters,
	DynamicSettings,
	ImageRequestProvider,
	DeepLinkHandler,
	BaseUrlProvider
);
