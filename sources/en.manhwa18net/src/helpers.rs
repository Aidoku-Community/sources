use crate::models::{InertiaPage, Paginate, prop};
use aidoku::{
	MangaPageResult, Result,
	alloc::{String, format},
	imports::net::Request,
	prelude::*,
};
use serde_json::Value;

pub const BASE_URL: &str = "https://manhwa18.net";

/// Turns a possibly relative image/link url into an absolute one.
pub fn absolute_url(url: &str) -> Option<String> {
	let url = url.trim();
	if url.is_empty() || url.starts_with("data:") {
		None
	} else if url.starts_with("http://") || url.starts_with("https://") {
		Some(url.into())
	} else if let Some(rest) = url.strip_prefix("//") {
		Some(format!("https://{rest}"))
	} else if url.starts_with('/') {
		Some(format!("{BASE_URL}{url}"))
	} else {
		Some(format!("{BASE_URL}/{url}"))
	}
}

/// Splits a chapter name such as `Chapter 12.5 - Title` into its number and title.
///
/// The number is looked for after the word "chapter" first, so that names like
/// `Season 2 Chapter 5` yield 5, and falls back to the first number in the name.
pub fn parse_chapter_name(name: &str) -> (Option<f32>, Option<String>) {
	let name = name.trim();
	let lower = name.to_ascii_lowercase();
	let from = lower.find("chapter").map_or(0, |i| i + "chapter".len());

	let bytes = name.as_bytes();
	let Some(start) = (from..bytes.len()).find(|&i| bytes[i].is_ascii_digit()) else {
		return (None, (!name.is_empty()).then(|| name.into()));
	};
	let mut end = start;
	while end < bytes.len() && bytes[end].is_ascii_digit() {
		end += 1;
	}
	if end + 1 < bytes.len() && bytes[end] == b'.' && bytes[end + 1].is_ascii_digit() {
		end += 1;
		while end < bytes.len() && bytes[end].is_ascii_digit() {
			end += 1;
		}
	}

	let number = name[start..end].parse::<f32>().ok();
	let title = name[end..].trim_start_matches(|c: char| c.is_whitespace() || "-:–—.".contains(c));
	(number, (!title.is_empty()).then(|| title.into()))
}

/// Extracts the manga slug and optional chapter slug from a site url.
pub fn parse_link(url: &str) -> Option<(&str, Option<&str>)> {
	let path = url.split(['?', '#']).next()?;
	let rest = path.split_once("/manga/")?.1;
	let mut parts = rest.split('/').filter(|part| !part.is_empty());
	let manga = parts.next()?;
	Some((manga, parts.next()))
}

/// Fetches a page of the site and returns the props of its embedded Inertia payload.
pub fn fetch_props(url: &str) -> Result<Value> {
	let html = Request::get(url)?.html()?;
	let Some(data) = html
		.select_first("#app")
		.and_then(|app| app.attr("data-page"))
		.filter(|data| !data.trim().is_empty())
	else {
		bail!("Couldn't find the page data, the site may be asking for a browser check");
	};
	Ok(serde_json::from_str::<InertiaPage>(&data)?.props)
}

pub fn fetch_list(url: &str) -> Result<MangaPageResult> {
	let props = fetch_props(url)?;
	// which prop holds the listing depends on the page that was requested
	let listing = ["paginate", "popularManga", "mangas", "latestManhwaMain"]
		.into_iter()
		.find_map(|key| prop::<Paginate>(&props, key));
	let Some(listing) = listing else {
		bail!("No manga listing found in response");
	};
	Ok(listing.into_result())
}
