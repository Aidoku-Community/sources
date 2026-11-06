//! Chapter lists for korean series (合集).

use aidoku::{
	Chapter, Result,
	alloc::{String, Vec, format, string::ToString},
	prelude::*,
};

use crate::net::base_url;

/// Chapters open inside the series page, matching the site's own link format.
fn series_chapter_url(chapter: &str, sid: &str) -> Result<String> {
	Ok(format!(
		"{}/photos-slide-aid-{chapter}-sid-{sid}.html",
		base_url()?
	))
}

/// Reads the `Url::SeriesChapters` response:
/// `{"code":0,"list":[{"id":110116,"idx":1,"name":"傀儡 1-70話"},…],"total":99}`.
///
/// Each `id` is an ordinary gallery whose `photos-gallery` page serves the
/// images. Anything short of the complete list is an error, so a library
/// refresh never stores a series with missing chapters.
pub fn parse_chapters(json: &str, sid: &str) -> Result<Vec<Chapter>> {
	if json_number(json, "code") != Some(0) {
		bail!("chapter list refused for {sid}");
	}
	let Some(total) = json_number(json, "total") else {
		bail!("chapter list without total for {sid}");
	};
	let mut chapters: Vec<Chapter> = Vec::new();
	for object in json_objects_in(json, "list") {
		let (Some(id), Some(idx)) = (json_number(object, "id"), json_number(object, "idx")) else {
			bail!("chapter without id for {sid}");
		};
		let key = id.to_string();
		chapters.push(Chapter {
			url: Some(series_chapter_url(&key, sid)?),
			key,
			title: json_string(object, "name").filter(|name| !name.trim().is_empty()),
			chapter_number: Some(idx as f32),
			..Default::default()
		});
	}
	if chapters.is_empty() || chapters.len() as i64 != total {
		bail!("chapter list for {sid} has {} of {total}", chapters.len());
	}
	// Newest first, matching the app's display order.
	chapters.sort_by(|a, b| {
		b.chapter_number
			.partial_cmp(&a.chapter_number)
			.unwrap_or(core::cmp::Ordering::Equal)
	});
	Ok(chapters)
}

/// `"key":123` → 123. Only the first occurrence is read, so callers pass a
/// single object, or rely on the key appearing before any nested object.
fn json_number(json: &str, key: &str) -> Option<i64> {
	let marker = format!("\"{key}\":");
	let rest = json[json.find(&marker)? + marker.len()..].trim_start();
	let end = rest
		.char_indices()
		.find(|&(i, ch)| !(ch.is_ascii_digit() || (i == 0 && ch == '-')))
		.map_or(rest.len(), |(i, _)| i);
	rest[..end].parse().ok()
}

/// `"key":"…"` → the unescaped string.
fn json_string(json: &str, key: &str) -> Option<String> {
	let marker = format!("\"{key}\":\"");
	let rest = &json[json.find(&marker)? + marker.len()..];
	let mut escaped = false;
	for (i, ch) in rest.char_indices() {
		if escaped {
			escaped = false;
		} else if ch == '\\' {
			escaped = true;
		} else if ch == '"' {
			return Some(json_unescape(&rest[..i]));
		}
	}
	None
}

/// Top-level objects of the array under `key`, skipping braces inside strings.
fn json_objects_in<'a>(json: &'a str, key: &str) -> Vec<&'a str> {
	let mut objects = Vec::new();
	let marker = format!("\"{key}\":[");
	let Some(at) = json.find(&marker) else {
		return objects;
	};
	let body = &json[at + marker.len()..];
	let (mut depth, mut start) = (0usize, 0usize);
	let (mut in_string, mut escaped) = (false, false);
	for (i, ch) in body.char_indices() {
		if in_string {
			if escaped {
				escaped = false;
			} else if ch == '\\' {
				escaped = true;
			} else if ch == '"' {
				in_string = false;
			}
			continue;
		}
		match ch {
			'"' => in_string = true,
			'{' => {
				if depth == 0 {
					start = i;
				}
				depth += 1;
			}
			'}' if depth > 0 => {
				depth -= 1;
				if depth == 0 {
					objects.push(&body[start..=i]);
				}
			}
			']' if depth == 0 => break,
			_ => {}
		}
	}
	objects
}

fn json_unescape(raw: &str) -> String {
	let mut out = String::with_capacity(raw.len());
	let mut chars = raw.chars();
	while let Some(ch) = chars.next() {
		if ch != '\\' {
			out.push(ch);
			continue;
		}
		match chars.next() {
			Some('n') => out.push('\n'),
			Some('t') => out.push('\t'),
			Some('r') => out.push('\r'),
			Some('u') => {
				let high = read_hex4(&mut chars);
				let code = match high {
					Some(h) if (0xD800..0xDC00).contains(&h) => {
						// A surrogate pair arrives as two `\uXXXX` escapes.
						// Peek ahead so a lone high surrogate does not
						// consume the next character.
						let mut ahead = chars.clone();
						let low = (ahead.next() == Some('\\') && ahead.next() == Some('u'))
							.then(|| read_hex4(&mut ahead))
							.flatten()
							.filter(|l| (0xDC00..0xE000).contains(l));
						if low.is_some() {
							chars = ahead;
						}
						// Broken pairs are dropped rather than misdecoded.
						low.map(|l| 0x10000 + ((h - 0xD800) << 10) + (l - 0xDC00))
					}
					other => other,
				};
				if let Some(c) = code.and_then(char::from_u32) {
					out.push(c);
				}
			}
			Some(other) => out.push(other),
			None => {}
		}
	}
	out
}

fn read_hex4(chars: &mut core::str::Chars) -> Option<u32> {
	let digits: String = chars.by_ref().take(4).collect();
	u32::from_str_radix(&digits, 16).ok()
}
