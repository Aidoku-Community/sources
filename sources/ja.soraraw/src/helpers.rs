use aes::{
	Aes256,
	cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray},
};
use aidoku::{
	AidokuError, ContentRating, MangaStatus, Result, Viewer,
	alloc::{String, Vec, string::ToString},
	imports::{html::Html, net::Request},
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::{BASE_URL, HEADER_BYTES, STACKED_PAGE_LIMIT, THUMBNAIL_URL, models::NextData};

const BLOCK_SIZE: usize = 16;

pub fn manga_url(slug: &str) -> String {
	format!("{BASE_URL}/manga/{slug}")
}

// chapter paths repeat the slug of their manga, which the url doesn't
pub fn chapter_url(manga_slug: &str, path: &str) -> String {
	let suffix = path.strip_prefix(&format!("{manga_slug}-")).unwrap_or(path);
	format!("{BASE_URL}/manga/{manga_slug}/{suffix}")
}

// both ids the image endpoint takes, so pages never rely on a slug carrying its id
pub fn chapter_key(manga_id: i64, chapter_id: i64) -> String {
	format!("{manga_id}/{chapter_id}")
}

pub fn paginated(url: &str, page: i32) -> String {
	if page > 1 {
		format!("{url}/page/{page}")
	} else {
		String::from(url)
	}
}

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

// synopses hold inline markup, which the app doesn't render
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

fn image_size(url: &str) -> Option<(u32, u32)> {
	let range = format!("bytes=0-{}", HEADER_BYTES - 1);
	let head = Request::get(url)
		.ok()?
		.header("Range", range.as_str())
		.data()
		.ok()?;
	jpeg_size(&head).or_else(|| webp_size(&head))
}

// a few chapters ship as one image stacking every page of the chapter, up to 49152 pixels tall.
// the scans are b5, so one page stands its width times √2, and every stacked chapter measured
// divides into a whole number of them
pub fn stacked_page_count(url: &str) -> u32 {
	let Some((width, height)) = image_size(url) else {
		return 1;
	};
	slice_count(width, height)
}

pub fn slice_count(width: u32, height: u32) -> u32 {
	let page_height = width as f32 * core::f32::consts::SQRT_2;
	// `f32::round` is not in core. a width of zero divides into infinity, which saturates to
	// `u32::MAX` and falls past the limit below rather than wrapping
	let count = ((height as f32 / page_height) + 0.5) as u32;
	if count > STACKED_PAGE_LIMIT {
		return 1;
	}
	count.max(1)
}

// neither extension says what the container is, so the magic bytes are what decides
pub fn jpeg_size(head: &[u8]) -> Option<(u32, u32)> {
	fn length(head: &[u8], at: usize) -> Option<usize> {
		Some(usize::from(u16::from_be_bytes([
			*head.get(at)?,
			*head.get(at + 1)?,
		])))
	}

	if *head.first()? != 0xFF || *head.get(1)? != 0xD8 {
		return None;
	}

	let mut index = 2;
	while *head.get(index)? == 0xFF {
		match *head.get(index + 1)? {
			// padding written ahead of the next marker
			0xFF => index += 1,
			// markers standing on their own, without a segment behind them
			0x01 | 0xD0..=0xD9 => index += 2,
			// a frame header, which opens with the precision and then the size of the image
			0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
				let height = length(head, index + 5)?.try_into().ok()?;
				let width = length(head, index + 7)?.try_into().ok()?;
				return Some((width, height));
			}
			_ => index += 2 + length(head, index + 2)?,
		}
	}

	None
}

// only the simple lossy form is read: a webp canvas stops at 16383 pixels, so the stacked images
// tall enough to need slicing are written as jpeg, and every webp the site serves is a `VP8 `
pub fn webp_size(head: &[u8]) -> Option<(u32, u32)> {
	if !head.starts_with(b"RIFF")
		|| head.get(8..12)? != b"WEBP".as_slice()
		|| head.get(12..16)? != b"VP8 ".as_slice()
		// the keyframe start code, which the frame size sits behind
		|| head.get(23..26)? != [0x9D, 0x01, 0x2A].as_slice()
	{
		return None;
	}

	// 14 bits of size and 2 of upscaling, which says nothing about how large the frame is
	let size = |at: usize| -> Option<u32> {
		let value = u16::from_le_bytes([*head.get(at)?, *head.get(at + 1)?]);
		Some(u32::from(value & 0x3FFF))
	};
	Some((size(26)?, size(28)?))
}

// listings hand out either a full cover url or just the file name on the thumbnail host
pub fn cover(thumbnail: Option<String>, image: Option<&str>) -> Option<String> {
	thumbnail
		.filter(|thumbnail| !thumbnail.is_empty())
		.or_else(|| {
			image
				.filter(|image| !image.is_empty())
				.map(|image| format!("{THUMBNAIL_URL}/{image}"))
		})
}

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

// the `mode` field says how the site's own reader lays a series out, not what kind of comic it
// is: a third of the `vertical` ones are ordinary manga. the overseas genres track the content
// instead, appearing on 644 of 979 `vertical` entries and on 2 of 7021 `horizontal` ones.
// image proportions don't work either, since webtoons here are as often cut into page-shaped
// chunks as into tall strips
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

// deriving anything further from genres was tried and dropped: names are not unique (41 of the
// 1834 listed are shared), and the suggestive ones are already flagged as adult by the site
pub fn content_rating(is_adult: Option<&str>) -> ContentRating {
	match is_adult {
		Some("yes") => ContentRating::NSFW,
		Some("no") => ContentRating::Safe,
		_ => ContentRating::Unknown,
	}
}

// comparing bytes is safe for utf-8: a multi-byte character can never match part of another one,
// so a byte window that compares equal is always a real substring
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

// both alphabets and optional padding are accepted, the same way the site's own decoder does
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

// the image endpoint hands out its page list xored with a fixed key
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

// page paths are xored with a fixed secret and encrypted with aes-256-ctr, keyed on the uuid the
// chapter page carries. building the path out of the ids instead only holds for part of the site:
// the file extension varies per chapter, and guessing it leaves whole series answering 404
pub fn decrypt_path(payload: &str, uuid: &str, secret: &[u8]) -> Option<String> {
	if secret.is_empty() {
		return None;
	}
	let key = decode_hex(uuid)?;
	if key.len() != 32 {
		return None;
	}

	let mut bytes = decode_base64(payload)?;
	// the value opens with the counter the stream starts at, so anything shorter holds no path
	if bytes.len() <= BLOCK_SIZE {
		return None;
	}
	for (index, byte) in bytes.iter_mut().enumerate() {
		*byte ^= secret[index % secret.len()];
	}

	let cipher = Aes256::new(GenericArray::from_slice(&key));
	let mut counter = [0u8; BLOCK_SIZE];
	counter.copy_from_slice(&bytes[..BLOCK_SIZE]);

	let mut path = Vec::with_capacity(bytes.len() - BLOCK_SIZE);
	for chunk in bytes[BLOCK_SIZE..].chunks(BLOCK_SIZE) {
		let mut block = GenericArray::from(counter);
		cipher.encrypt_block(&mut block);
		for (byte, mask) in chunk.iter().zip(block.iter()) {
			path.push(byte ^ mask);
		}
		increment(&mut counter);
	}

	String::from_utf8(path).ok().filter(|path| !path.is_empty())
}

fn increment(counter: &mut [u8; BLOCK_SIZE]) {
	for byte in counter.iter_mut().rev() {
		*byte = byte.wrapping_add(1);
		if *byte != 0 {
			break;
		}
	}
}

fn decode_hex(input: &str) -> Option<Vec<u8>> {
	if !input.len().is_multiple_of(2) {
		return None;
	}

	let mut output = Vec::with_capacity(input.len() / 2);
	let mut bytes = input.bytes();
	while let (Some(high), Some(low)) = (bytes.next(), bytes.next()) {
		let digit = |byte: u8| match byte {
			b'0'..=b'9' => Some(byte - b'0'),
			b'a'..=b'f' => Some(byte - b'a' + 10),
			b'A'..=b'F' => Some(byte - b'A' + 10),
			_ => None,
		};
		output.push(digit(high)? << 4 | digit(low)?);
	}

	Some(output)
}
