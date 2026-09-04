#![no_std]
extern crate alloc;

use core::cell::RefCell;

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

struct XComic {
	latest_cursor: RefCell<Option<i64>>,
}

impl XComic {
	fn latest_page(
		&self,
		base_url: &str,
		params: &BrowseParams,
	) -> Result<(Vec<models::ComicData>, bool)> {
		let mut before = if params.page == 1 {
			None
		} else {
			*self.latest_cursor.borrow()
		};
		for _ in 0..10 {
			let response = graphql::latest_uploads_request(base_url, before)?.send()?;
			let (entries, next_cursor) = graphql::parse_latest_uploads(response, params)?;
			// Only the home page pairs these with their chapter.
			let comics: Vec<models::ComicData> =
				entries.into_iter().map(|(comic, _)| comic).collect();
			let has_next_page = next_cursor.is_some();
			*self.latest_cursor.borrow_mut() = next_cursor;
			if !comics.is_empty() || !has_next_page {
				return Ok((comics, has_next_page));
			}
			before = next_cursor;
		}
		Ok((Vec::new(), self.latest_cursor.borrow().is_some()))
	}

	/// One page of browse results, shared by search and listings.
	fn browse_page(&self, base_url: &str, params: BrowseParams) -> Result<MangaPageResult> {
		let (comics, has_next_page) = if params.can_use_latest_uploads() {
			self.latest_page(base_url, &params)?
		} else {
			let response = graphql::browse_request(base_url, &params)?.send()?;
			graphql::parse_browse(response, &params)?
		};
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
		Self {
			latest_cursor: RefCell::new(None),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base_url = self.get_base_url()?;
		// Quick open: a pasted url or an `id:<value>` query resolves directly.
		if let Some(target) = query.as_deref().and_then(helpers::target_from_query) {
			let key = helpers::comic_key(&base_url, target)?;
			let comic = graphql::fetch_comic(&base_url, &key)?;
			return Ok(MangaPageResult {
				entries: vec![manga_from_data(comic, &base_url)],
				has_next_page: false,
			});
		}
		let mut params = BrowseParams::new("field_score", page)?;
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
					"upload_status" => params.upload_status = value,
					"chapter_count" => params.chapter_count = value,
					"include_mode" => params.include_mode = value,
					"exclude_mode" => params.exclude_mode = value,
					_ => {}
				},
				FilterValue::Text { id, value } if id == "year" => {
					(params.year_min, params.year_max) = helpers::parse_year(&value);
				}
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
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let base_url = self.get_base_url()?;
		if needs_details {
			let comic = graphql::fetch_comic(&base_url, &manga.key)?;
			let chapters = manga.chapters.take();
			manga = manga_from_data(comic, &base_url);
			manga.chapters = chapters;
			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			manga.chapters = Some(
				graphql::fetch_chapters(&base_url, &manga.key)?
					.into_iter()
					.filter_map(|chapter| chapter_from_data(chapter, &base_url, None, false))
					.collect(),
			);
		}
		Ok(manga)
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
		self.browse_page(&base_url, BrowseParams::new(&listing.id, page)?)
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
