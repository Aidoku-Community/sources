use aidoku::{
	Link, Manga, Result,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::models::MangaListItemDto;

/// Olympus keeps this page's og:url pointed at its current domain.
const DOMAIN_FINDER_URL: &str = "https://olympus.pages.dev";

/// Manga keys are the series id. Slugs aren't used in keys because they change
/// over time.
pub fn parse_manga_id(key: &str) -> Result<i32> {
	key.parse()
		.map_err(|_| error!("Clave de serie no válida: {key}"))
}

pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?.json_owned()
}

pub fn rankings_url(base_url: &str, period: &str, page: i32) -> String {
	format!("{base_url}/api/rankings?page={page}&period={period}")
}

pub fn latest_url(base_url: &str, page: i32) -> String {
	format!("{base_url}/api/new-chapters?page={page}")
}

pub fn manga_links(items: Vec<MangaListItemDto>) -> Vec<Link> {
	items
		.into_iter()
		.filter(MangaListItemDto::is_comic)
		.map(|item| Manga::from(item).into())
		.collect()
}

/// Follows the same steps as the keiyoushi extension: read the og:url of
/// olympus.pages.dev, then follow that domain's redirects to the final one. The
/// og:url domain itself only redirects, even for /api/ requests.
pub fn detect_base_url() -> Option<String> {
	let og_url = Request::get(DOMAIN_FINDER_URL)
		.ok()?
		.html()
		.ok()?
		.select_first("meta[property=og:url]")?
		.attr("content")?;
	let final_url = Request::head(&og_url).ok()?.send().ok()?.get_url()?;

	let (scheme, rest) = final_url.split_once("://")?;
	let host = rest.split('/').next().filter(|host| !host.is_empty())?;
	Some(format!("{scheme}://{host}"))
}
