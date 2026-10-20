#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec},
	prelude::*,
};
use core::cell::RefCell;

mod helpers;
mod models;
mod search;
pub use helpers::absolute_url;
pub(crate) use helpers::fetch_props;
use helpers::{browse_url, fetch_list, find_artist_id, manga_url, reader_url, slug_title};
use models::*;
use search::{NameIds, NameSearch, SearchParams};

pub const BASE_URL: &str = "https://hentai2read.com";

/// Tag page kinds, e.g. `/parody/751-kantai-collection`, that open as listings.
pub const TAG_KINDS: [&str; 6] = [
	"artist",
	"parody",
	"character",
	"publisher",
	"content",
	"category",
];

/// Sort filter values, used as the search `sort` and the browse path segment. Browsing has no
/// relevance, so an empty sort falls back to recently updated.
const SORTS: [&str; 6] = [
	"",
	"latest",
	"newest",
	"most-viewed",
	"top-rated",
	"alphabetical",
];

struct Hentai2Read {
	/// The current name search, continued by the following pages.
	name_search: RefCell<Option<NameSearch>>,
	/// Ids of the tags, artists and parodies named exactly like earlier searches.
	name_ids: RefCell<NameIds>,
	/// Artist ids found for names typed in the artist filter.
	artist_ids: RefCell<NameIds>,
}

impl Source for Hentai2Read {
	fn new() -> Self {
		Self {
			name_search: RefCell::new(None),
			name_ids: RefCell::new(NameIds::default()),
			artist_ids: RefCell::new(NameIds::default()),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut params = SearchParams {
			kind: String::new(),
			status: String::new(),
			sort: "",
			with: Vec::new(),
			without: Vec::new(),
			match_any: false,
		};
		let mut sort = 0;
		let mut artist_id = None;
		let mut tapped = None;

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } if id == "author" => {
					let name = value.trim();
					if !name.is_empty() {
						match find_artist_id(name, &mut self.artist_ids.borrow_mut())? {
							Some(id) => artist_id = Some(id),
							None => return Ok(MangaPageResult::default()),
						}
					}
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"type" => params.kind = value,
					"status" => params.status = value,
					// Sent when a tag is tapped on a series page.
					"genre" => tapped = Some(value),
					_ => {}
				},
				FilterValue::Sort { index, .. } => sort = usize::try_from(index).unwrap_or(0),
				FilterValue::Check { id, value } if id == "any" => params.match_any = value == 1,
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "tags" => {
					params.with = included;
					params.without = excluded;
				}
				_ => {}
			}
		}

		// The artist is sent as a tag, so it would become optional when matching any tag.
		if let Some(id) = artist_id {
			params.with.push(id);
			params.match_any = false;
		}

		params.sort = SORTS.get(sort).copied().unwrap_or_default();
		let query = query.as_deref().map(str::trim).unwrap_or_default();

		// Browsing paginates deeper than search, which is capped at 1000 results.
		if query.is_empty()
			&& tapped.is_none()
			&& params.with.is_empty()
			&& params.without.is_empty()
		{
			return fetch_list(&browse_url(
				&[&params.kind, &params.status, params.sort],
				page,
			));
		}

		if query.is_empty() && tapped.is_none() {
			return fetch_list(&params.url("", None, page));
		}

		let key = NameSearch::key(&params, query, tapped.as_deref());
		let mut name_search = self.name_search.borrow_mut();
		if !name_search
			.as_ref()
			.is_some_and(|search| search.is_continued_by(&key, page))
		{
			*name_search = Some(NameSearch::new(
				key,
				params,
				query,
				tapped.as_deref(),
				&mut self.name_ids.borrow_mut(),
			)?);
		}
		name_search
			.as_mut()
			.expect("name search is initialized above")
			.next()
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let BookProps {
			book,
			chapters,
			stats,
		} = fetch_props(&manga_url(&manga.key))?;

		if needs_chapters {
			manga.chapters = Some(
				chapters
					.into_iter()
					.map(|chapter| chapter.into_chapter(&book.title))
					.collect(),
			);
		}
		if needs_details {
			book.apply_to(stats, &mut manga);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let props: ReaderProps = fetch_props(&reader_url(&manga.key, &chapter.key))?;
		Ok(props
			.images
			.iter()
			.map(|path| Page {
				content: PageContent::url(absolute_url(path)),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for Hentai2Read {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		// Tag pages from `handle_deep_link`, e.g. `/parody/751-kantai-collection`. Series
		// descriptions link each artist, parody, character and publisher to its tag page, and
		// tapping one of those links opens it through the deep link handler.
		if listing.id.starts_with('/') {
			return fetch_list(&format!("{BASE_URL}{}/page-{page}", listing.id));
		}
		fetch_list(&browse_url(&[&listing.id], page))
	}
}

impl DeepLinkHandler for Hentai2Read {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let mut segments = path.split('/').filter(|segment| !segment.is_empty());

		match (segments.next(), segments.next(), segments.next()) {
			(Some("reader"), Some(slug), Some(chapter_id)) => {
				// Reader urls don't include the series type, so the series url is read from the page.
				let key = format!("/reader/{slug}/{chapter_id}");
				let props: ReaderProps = fetch_props(&format!("{BASE_URL}{key}/page-1"))?;
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: props.book.url,
					key,
				}))
			}
			(Some(kind @ ("original" | "doujinshi" | "imageset")), Some(slug), None) => {
				Ok(Some(DeepLinkResult::Manga {
					key: format!("/{kind}/{slug}"),
				}))
			}
			(Some(kind), Some(slug), None) if TAG_KINDS.contains(&kind) => {
				Ok(Some(DeepLinkResult::Listing(Listing {
					id: format!("/{kind}/{slug}"),
					name: slug_title(slug),
					..Default::default()
				})))
			}
			_ => Ok(None),
		}
	}
}

register_source!(Hentai2Read, ListingProvider, DeepLinkHandler);
