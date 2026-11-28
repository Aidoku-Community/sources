#![no_std]
mod api;
mod filters;
mod helpers;
mod home;
mod models;
mod settings;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec, format, vec},
	imports::{
		net::{Request, TimeUnit, set_rate_limit},
		std::{current_date, send_partial_result},
	},
	prelude::*,
};
use api::{BASE_URL, SORTS, SearchParams, chapter_list, parse, rpc, set_age};
use core::cell::{Cell, RefCell};
use helpers::{Link, page_expired, page_fragment, parse_link, parse_terms};
use models::{Bootstrap, PageImage, TermOption, WorkDetail, Years};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use settings::{adult, deduplicate_chapters, languages};

const MAX_CURSORS: usize = 64;

struct OniSaga {
	cursors: RefCell<Vec<(String, String)>>,
	age: Cell<Option<bool>>,
	pages: RefCell<Option<(String, Vec<PageImage>)>>,
	pub(crate) taxonomy: RefCell<Option<Vec<TermOption>>>,
}

impl OniSaga {
	pub(crate) fn sync_age(&self, adult: bool) -> Result<()> {
		if self.age.get() != Some(adult) {
			set_age(adult)?;
			self.age.set(Some(adult));
		}
		Ok(())
	}

	fn page(&self, mut search: SearchParams, page: i32) -> Result<MangaPageResult> {
		search.finalize();
		if search.is_empty() {
			return Ok(MangaPageResult::default());
		}
		self.sync_age(adult())?;
		let listing_key = search.key()?;
		if page > 1 {
			let key = format!("{page}:{listing_key}");
			let cursor = self
				.cursors
				.borrow()
				.iter()
				.find(|(k, _)| *k == key)
				.map(|(_, c)| c.clone());
			let Some(cursor) = cursor else {
				return Ok(MangaPageResult::default());
			};
			search.cursor = Some(cursor);
		}
		let (result, next) = search.page()?;
		if let Some(cursor) = next {
			let key = format!("{}:{listing_key}", page + 1);
			let mut cursors = self.cursors.borrow_mut();
			if let Some((_, existing)) = cursors.iter_mut().find(|(k, _)| *k == key) {
				*existing = cursor;
			} else {
				if cursors.len() >= MAX_CURSORS {
					cursors.remove(0);
				}
				cursors.push((key, cursor));
			}
		}
		Ok(result)
	}

	// Adult titles answer 403 or 404 without the age cookie, so saved and linked ones confirm it.
	fn gated<T: DeserializeOwned>(&self, method: &str, input: Value) -> Result<T> {
		let response = rpc(method, &input)?.send()?;
		if matches!(response.status_code(), 403 | 404) && self.age.get() != Some(true) {
			self.sync_age(true)?;
			return parse(rpc(method, input)?.send()?);
		}
		parse(response)
	}

	fn detail(&self, slug: &str) -> Result<WorkDetail> {
		self.gated("catalog/detail", json!({ "slug": slug }))
	}

	fn bootstrap(&self, release_id: &str) -> Result<Vec<PageImage>> {
		let bootstrap: Bootstrap =
			self.gated("reader/bootstrap", json!({ "releaseId": release_id }))?;
		let pages = bootstrap.into_pages()?;
		*self.pages.borrow_mut() = Some((release_id.into(), pages.clone()));
		Ok(pages)
	}
}

impl Source for OniSaga {
	fn new() -> Self {
		set_rate_limit(3, 1, TimeUnit::Seconds);
		Self {
			cursors: RefCell::new(Vec::new()),
			age: Cell::new(None),
			pages: RefCell::new(None),
			taxonomy: RefCell::new(None),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(target) = query.as_deref().and_then(parse_link) {
			let (Link::Title(slug) | Link::Chapter(slug, _)) = target;
			if let Ok(detail) = self.detail(&slug)
				&& let Ok(manga) = detail.into_basic_manga()
			{
				return Ok(MangaPageResult {
					entries: vec![manga],
					has_next_page: false,
				});
			}
		}

		let mut search = SearchParams::new(query);
		let mut tapped = Vec::new();
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(sort) = SORTS.get(index as usize) {
						search.sort = sort;
					}
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"origin" => search.origin = Some(included),
					"genre" => {
						search.genres.include.extend(included);
						search.genres.exclude.extend(excluded);
					}
					"status" => search.status = included,
					"form" => search.form = included,
					"demographic" => search.demographic = included,
					_ => {}
				},
				FilterValue::Select { id, value } if !value.is_empty() && id == "genre" => {
					tapped.push(value);
				}
				FilterValue::Text { id, value } => match id.as_str() {
					"theme" => parse_terms(
						&value,
						&mut search.themes.include,
						&mut search.themes.exclude,
					),
					"tag" => {
						parse_terms(&value, &mut search.tags.include, &mut search.tags.exclude)
					}
					_ => {}
				},
				FilterValue::Range { id, from, to } if id == "year" => {
					let (from, to) = (from.map(|y| y as i32), to.map(|y| y as i32));
					if let (Some(start), Some(end)) = (from, to)
						&& start > end
					{
						bail!("Start year cannot exceed end year");
					}
					if from.is_some() || to.is_some() {
						search.year = Some(Years { from, to });
					}
				}
				FilterValue::Check { id, value } if id == "readable" => {
					search.readable = value == 1
				}
				_ => {}
			}
		}
		if !tapped.is_empty() {
			self.resolve_tapped_tags(tapped, &mut search);
		}
		self.page(search, page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut detail = self.detail(&manga.key)?;
		let reading = core::mem::take(&mut detail.reading);

		if needs_details {
			manga.copy_from(Manga::try_from(detail)?);
		} else {
			manga.key = detail.slug;
		}
		if !needs_chapters {
			return Ok(manga);
		}

		let scopes = reading.language_scopes(&languages());
		if scopes.is_empty() {
			manga.chapters = Some(Vec::new());
			return Ok(manga);
		}
		if needs_details {
			send_partial_result(&manga);
		}
		manga.chapters = Some(chapter_list(
			&manga.key,
			&reading,
			&scopes,
			deduplicate_chapters(),
		)?);
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Ok(self
			.bootstrap(&chapter.key)?
			.into_iter()
			.map(|p| Page {
				content: PageContent::url(format!("{}#oni={}.{}", p.url, chapter.key, p.ord)),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for OniSaga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let search = match listing.id.as_str() {
			"updated" => SearchParams::sorted("updated"),
			"new" => SearchParams::sorted("new"),
			"finished" => SearchParams {
				status: vec!["completed".into()],
				..SearchParams::sorted("updated")
			},
			"published" => SearchParams::sorted("published"),
			_ => bail!("Unknown listing"),
		};
		self.page(search, page)
	}
}

impl ImageRequestProvider for OniSaga {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let resolved = match page_fragment(&url) {
			Some((clean, release_id, ord)) if page_expired(clean, current_date()) => self
				.pages
				.borrow()
				.as_ref()
				.filter(|(id, _)| id == release_id)
				.and_then(|(_, pages)| {
					let found = pages.iter().find(|p| p.ord == ord)?;
					(!page_expired(&found.url, current_date())).then(|| found.url.clone())
				})
				.or_else(|| {
					self.bootstrap(release_id)
						.ok()?
						.into_iter()
						.find(|p| p.ord == ord)
						.map(|p| p.url)
				})
				.unwrap_or_else(|| clean.into()),
			Some((clean, ..)) => clean.into(),
			None => url,
		};
		Ok(Request::get(resolved)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for OniSaga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(parse_link(&url).map(|target| match target {
			Link::Title(key) => DeepLinkResult::Manga { key },
			Link::Chapter(manga_key, key) => DeepLinkResult::Chapter { manga_key, key },
		}))
	}
}

register_source!(
	OniSaga,
	Home,
	ListingProvider,
	DynamicFilters,
	DynamicSettings,
	ImageRequestProvider,
	DeepLinkHandler
);
