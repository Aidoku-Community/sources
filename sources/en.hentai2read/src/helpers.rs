use crate::{BASE_URL, models::*, search::NameIds};
use aidoku::{
	MangaPageResult, Result,
	alloc::{String, Vec, string::ToString},
	helpers::uri::encode_uri_component,
	imports::net::Request,
	prelude::*,
};
use serde::de::DeserializeOwned;

pub fn absolute_url(path: &str) -> String {
	if path.starts_with("https://") || path.starts_with("http://") {
		path.into()
	} else {
		format!("{BASE_URL}{path}")
	}
}

// Keys from the previous version of this source are old site slugs (`one_hurricane`) and chapter
// numbers (`8`). The site redirects those old urls to the current pages, so they still resolve.
pub(super) fn manga_url(key: &str) -> String {
	if key.starts_with('/') {
		absolute_url(key)
	} else {
		format!("{BASE_URL}/{key}/")
	}
}

pub(super) fn reader_url(manga_key: &str, chapter_key: &str) -> String {
	if chapter_key.starts_with('/') {
		format!("{BASE_URL}{chapter_key}/page-1")
	} else {
		format!("{BASE_URL}/{manga_key}/{chapter_key}/")
	}
}

/// The site is an Inertia.js app, so every page embeds its full data as JSON.
pub(crate) fn fetch_props<T: DeserializeOwned>(url: &str) -> Result<T> {
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
pub(super) fn find_artist_id(name: &str, artist_ids: &mut NameIds) -> Result<Option<String>> {
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

pub(super) fn fetch_list(url: &str) -> Result<MangaPageResult> {
	Ok(fetch_props::<ListProps>(url)?.books.into())
}

/// Browse pages are paginated by path, e.g. `/browse/doujinshi/completed/most-viewed/page-2`.
pub(super) fn browse_url(segments: &[&str], page: i32) -> String {
	let mut url = format!("{BASE_URL}/browse");
	for segment in segments.iter().filter(|segment| !segment.is_empty()) {
		url.push('/');
		url.push_str(segment);
	}
	format!("{url}/page-{page}")
}

/// Turns a tag slug like `751-kantai-collection` into a title like `Kantai Collection`.
pub(super) fn slug_title(slug: &str) -> String {
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
