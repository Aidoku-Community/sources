use aidoku::{
	AidokuError, ContentRating, MangaStatus, Result, Viewer,
	alloc::{String, Vec, string::ToString},
	imports::{html::Html, net::Request},
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::{BASE_URL, THUMBNAIL_URL, models::NextData};

pub fn manga_url(slug: &str) -> String {
	format!("{BASE_URL}/manga/{slug}")
}

/// Chapter paths repeat the slug of their manga, which the url they're reachable at doesn't.
pub fn chapter_url(manga_slug: &str, path: &str) -> String {
	let suffix = path.strip_prefix(&format!("{manga_slug}-")).unwrap_or(path);
	format!("{BASE_URL}/manga/{manga_slug}/{suffix}")
}

/// Chapter keys hold both ids the image endpoint takes, so requesting pages never has to rely on
/// the slug of a manga carrying its id.
pub fn chapter_key(manga_id: i64, chapter_id: i64) -> String {
	format!("{manga_id}/{chapter_id}")
}

/// Listing pages put the page number in the path, and only from the second page on.
pub fn paginated(url: &str, page: i32) -> String {
	if page > 1 {
		format!("{url}/page/{page}")
	} else {
		String::from(url)
	}
}

/// Reads the "__NEXT_DATA__" blob a page embeds, which holds everything it renders from.
pub fn next_data<T: DeserializeOwned>(url: &str) -> Result<T> {
	let html = Request::get(url)?.html()?;
	// script contents are data nodes rather than text, so `text` would come back empty. `data` is
	// what the app implements it with; the test runner only answers `html`, which holds the same
	// string for a script tag
	let Some(json) = html
		.select_first("script#__NEXT_DATA__")
		.and_then(|script| script.data().or_else(|| script.html()))
	else {
		bail!("no page data at {url}");
	};

	serde_json::from_str::<NextData<T>>(&json)
		.map(|data| data.props.page_props)
		.map_err(|error| AidokuError::Message(format!("unexpected page data at {url}: {error}")))
}

/// Synopses hold inline markup, which the app doesn't render.
pub fn strip_html(text: &str) -> String {
	// wrapped in an element of its own: reading the text off a bare fragment works in the app but
	// not in the test runner, which only ever hands back elements a selector matched
	Html::parse_fragment(format!("<div>{text}</div>"))
		.ok()
		.and_then(|document| document.select_first("div"))
		.and_then(|element| element.text())
		.unwrap_or_else(|| String::from(text))
		.trim()
		.into()
}

/// Picks the extension the pages of a chapter are stored under.
///
/// Page file names only ever leave the server encrypted, so they're rebuilt from the id and the
/// position of each page, which the endpoint does hand out in the clear. That leaves the extension
/// open: of 50 chapters sampled across the catalogue, 47 held webp and 3 held jpg, with nothing in
/// the response telling the two apart. Asking for the first page is what keeps the jpg chapters
/// readable, and costs one head request per chapter.
pub fn image_extension(host: &str, chapter_id: i64, first_page: &str) -> &'static str {
	const DEFAULT: &str = "webp";
	const FALLBACK: &str = "jpg";

	let response = Request::head(format!("{host}/c{chapter_id}/{first_page}.{DEFAULT}"))
		.and_then(|request| request.send());
	match response {
		// only a missing file rules the default out; a request that failed outright, or a server
		// answering anything else, is better served by the extension almost every chapter uses
		Ok(response) if response.status_code() == 404 => FALLBACK,
		_ => DEFAULT,
	}
}

/// Listings hand out either a full cover url or just the file name on the thumbnail host.
pub fn cover(thumbnail: Option<String>, image: Option<&str>) -> Option<String> {
	thumbnail
		.filter(|thumbnail| !thumbnail.is_empty())
		.or_else(|| {
			image
				.filter(|image| !image.is_empty())
				.map(|image| format!("{THUMBNAIL_URL}/{image}"))
		})
}

/// Authors come as a single comma separated field.
pub fn authors(author: Option<&str>) -> Option<Vec<String>> {
	let authors = author?
		.split(',')
		.map(|author| String::from(author.trim()))
		.filter(|author| !author.is_empty())
		.collect::<Vec<String>>();
	(!authors.is_empty()).then_some(authors)
}

pub fn status(kind: Option<&str>) -> MangaStatus {
	match kind {
		Some("complete") => MangaStatus::Completed,
		Some("incomplete") => MangaStatus::Ongoing,
		_ => MangaStatus::Unknown,
	}
}

/// Picks the reader from the genres a series carries, which is the only field that tracks what the
/// artwork actually is.
///
/// The `mode` field looks like the obvious input and isn't: it says how the site's own reader lays
/// a series out, not what kind of comic it is. Measured across 31 series, every `horizontal` one
/// held page-shaped art — but so did a third of the `vertical` ones, ordinary japanese manga that
/// the app would otherwise open in a continuous scroll. The overseas genres track the content
/// instead: across 8000 catalogue entries they appear on 2 of 7021 `horizontal` series and on 644
/// of 979 `vertical` ones, which is the share of `vertical` series that really are webtoons.
///
/// Image proportions are deliberately not used either. Korean webtoons here are cut into
/// page-shaped chunks about as often as into tall strips, so the shape of a page says nothing
/// about whether the panels are meant to run together.
pub fn viewer<'a>(genre_slugs: impl Iterator<Item = &'a str>) -> Viewer {
	const OVERSEAS_GENRE: &str = "kaigai-manga";

	for slug in genre_slugs {
		if slug == OVERSEAS_GENRE || slug.contains("webtoon") {
			return Viewer::Webtoon;
		}
	}
	// everything else on a japanese raw site reads as manga
	Viewer::RightToLeft
}

/// Every entry carries the flag the site sorts adult content by, which is what this follows.
///
/// Deriving anything further from the genres of a series was tried and dropped: genre names are
/// not unique (41 of the 1834 the site lists are used by more than one genre), and the ones that
/// read as suggestive are already flagged as adult by the site itself, so a name based guess
/// disagreed with the site more often than it added anything.
pub fn content_rating(is_adult: Option<&str>) -> ContentRating {
	match is_adult {
		Some("yes") => ContentRating::NSFW,
		Some("no") => ContentRating::Safe,
		_ => ContentRating::Unknown,
	}
}

/// Case insensitive substring search that doesn't allocate.
///
/// Comparing bytes is safe for the utf-8 the catalogue holds: a multi-byte character can never
/// match part of another one, so a byte window that compares equal is always a real substring.
pub fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
	let needle = needle.as_bytes();
	if needle.is_empty() {
		return true;
	}
	haystack
		.as_bytes()
		.windows(needle.len())
		.any(|window| window.eq_ignore_ascii_case(needle))
}

/// Decodes the base64 payload the image endpoint returns, accepting both the standard and the url
/// safe alphabet and treating padding as optional, the same way the site's own decoder does.
pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
	let mut output = Vec::with_capacity(input.len() / 4 * 3);
	let mut buffer = 0u32;
	let mut bits = 0u32;

	for byte in input.bytes() {
		let value = match byte {
			b'A'..=b'Z' => byte - b'A',
			b'a'..=b'z' => byte - b'a' + 26,
			b'0'..=b'9' => byte - b'0' + 52,
			b'+' | b'-' => 62,
			b'/' | b'_' => 63,
			b'=' => break,
			b' ' | b'\t' | b'\r' | b'\n' => continue,
			_ => return None,
		};
		buffer = (buffer << 6) | u32::from(value);
		bits += 6;
		if bits >= 8 {
			bits -= 8;
			output.push((buffer >> bits) as u8);
		}
	}

	Some(output)
}

/// Turns the payload of the image endpoint back into the json it was built from. The site xors it
/// with a fixed key, which is the only thing standing between the endpoint and a page list.
pub fn deobfuscate(payload: &str, key: &[u8]) -> Option<String> {
	if key.is_empty() {
		return None;
	}

	let mut bytes = decode_base64(payload)?;
	for (index, byte) in bytes.iter_mut().enumerate() {
		*byte ^= key[index % key.len()];
	}

	let json = String::from_utf8(bytes).ok()?;
	// the payloads carry a byte order mark and trailing padding that json won't accept
	let json = json
		.trim_start_matches('\u{feff}')
		.trim_matches(|char| char == '\0' || char::is_whitespace(char));
	(!json.is_empty()).then(|| json.to_string())
}
