#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString},
	helpers::uri::encode_uri_component,
	imports::net::Request,
	prelude::*,
};
use core::cell::RefCell;
use serde::de::DeserializeOwned;

mod models;
mod search;
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

/// Sort filter options as (search value, browse path segment).
const SORTS: [(&str, &str); 6] = [
	("", ""), // Relevance; browsing has no relevance, so it falls back to recently updated
	("latest", ""),
	("newest", "newest"),
	("most-viewed", "most-viewed"),
	("top-rated", "top-rated"),
	("alphabetical", "alphabetical"),
];

pub fn absolute_url(path: &str) -> String {
	if path.starts_with("https://") || path.starts_with("http://") {
		path.into()
	} else {
		format!("{BASE_URL}{path}")
	}
}

// Keys from the previous version of this source are old site slugs (`one_hurricane`) and chapter
// numbers (`8`). The site redirects those old urls to the current pages, so they still resolve.
fn manga_url(key: &str) -> String {
	if key.starts_with('/') {
		absolute_url(key)
	} else {
		format!("{BASE_URL}/{key}/")
	}
}

fn reader_url(manga_key: &str, chapter_key: &str) -> String {
	if chapter_key.starts_with('/') {
		format!("{BASE_URL}{chapter_key}/page-1")
	} else {
		format!("{BASE_URL}/{manga_key}/{chapter_key}/")
	}
}

/// The site is an Inertia.js app, so every page embeds its full data as JSON.
fn fetch_props<T: DeserializeOwned>(url: &str) -> Result<T> {
	let json = Request::get(url)?
		.html()?
		.select_first("script[data-page=app]")
		.and_then(|script| script.data())
		.ok_or_else(|| error!("No page data found at {url}"))?;
	serde_json::from_str::<InertiaPage<T>>(&json)
		.map(|page| page.props)
		.map_err(|err| error!("Unexpected page data at {url}: {err}"))
}

/// Searches are by tag id, so artist names typed in the artist filter are looked up through the
/// site's artist autocomplete, preferring an exact name over the first (most popular) result.
/// Results are remembered for the session.
fn find_artist_id(name: &str, artist_ids: &mut NameIds) -> Result<Option<String>> {
	if let Some(ids) = artist_ids.get(name) {
		return Ok(ids.first().cloned());
	}
	let artists: Vec<TagRef> = Request::get(format!(
		"{BASE_URL}/search/artists?q={}",
		encode_uri_component(name)
	))?
	.header("Accept", "application/json")
	.json_owned()?;
	let id = artists
		.iter()
		.find(|artist| artist.name.eq_ignore_ascii_case(name))
		.or(artists.first())
		.map(|artist| artist.id.to_string());
	artist_ids.insert(name, id.iter().cloned().collect());
	Ok(id)
}

fn fetch_list(url: &str) -> Result<MangaPageResult> {
	Ok(fetch_props::<ListProps>(url)?.books.into())
}

/// Browse pages are paginated by path, e.g. `/browse/doujinshi/completed/most-viewed/page-2`.
fn browse_url(segments: &[&str], page: i32) -> String {
	let mut url = format!("{BASE_URL}/browse");
	for segment in segments.iter().filter(|segment| !segment.is_empty()) {
		url.push('/');
		url.push_str(segment);
	}
	format!("{url}/page-{page}")
}

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
		let mut kind = String::new();
		let mut status = String::new();
		let mut sort = 0;
		let mut match_any = false;
		let mut with = Vec::new();
		let mut without = Vec::new();
		let mut artist = None;
		let mut tapped = None;

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } if id == "author" => artist = Some(value),
				FilterValue::Select { id, value } => match id.as_str() {
					"type" => kind = value,
					"status" => status = value,
					// Sent when a tag is tapped on a series page.
					"genre" => tapped = Some(value),
					_ => {}
				},
				FilterValue::Sort { index, .. } => sort = usize::try_from(index).unwrap_or(0),
				FilterValue::Check { id, value } if id == "any" => match_any = value == 1,
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "tags" => {
					with = included;
					without = excluded;
				}
				_ => {}
			}
		}

		if let Some(name) = artist
			.as_deref()
			.map(str::trim)
			.filter(|name| !name.is_empty())
		{
			match find_artist_id(name, &mut self.artist_ids.borrow_mut())? {
				Some(id) => with.push(id),
				None => return Ok(MangaPageResult::default()),
			}
			// The artist is sent as a tag, so it would become optional when matching any tag.
			match_any = false;
		}

		let (search_sort, browse_sort) = SORTS.get(sort).copied().unwrap_or_default();
		let query = query.as_deref().map(str::trim).unwrap_or_default();

		// Browsing paginates deeper than search, which is capped at 1000 results.
		if query.is_empty() && tapped.is_none() && with.is_empty() && without.is_empty() {
			return fetch_list(&browse_url(&[&kind, &status, browse_sort], page));
		}

		let params = SearchParams {
			kind,
			status,
			sort: search_sort,
			with,
			without,
			match_any,
		};
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
		match name_search.as_mut() {
			Some(search) => search.next(),
			None => Ok(MangaPageResult::default()),
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if !needs_details && !needs_chapters {
			return Ok(manga);
		}

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
		// Tag pages opened from description links, e.g. `/parody/751-kantai-collection`.
		if listing.id.starts_with('/') {
			return fetch_list(&format!("{BASE_URL}{}/page-{page}", listing.id));
		}
		let segment = match listing.id.as_str() {
			"latest" => "",
			id => id,
		};
		fetch_list(&browse_url(&[segment], page))
	}
}

/// Turns a tag slug like `751-kantai-collection` into a title like `Kantai Collection`.
fn slug_title(slug: &str) -> String {
	let name = slug.split_once('-').map_or(slug, |(_, name)| name);
	let mut title = String::with_capacity(name.len());
	for (index, word) in name.split('-').enumerate() {
		if index > 0 {
			title.push(' ');
		}
		let mut chars = word.chars();
		if let Some(first) = chars.next() {
			title.extend(first.to_uppercase());
			title.push_str(chars.as_str());
		}
	}
	title
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
