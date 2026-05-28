use aidoku::imports::{defaults::defaults_get, html::Document, net::Request};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const BASE_URL: &str = "https://myreadingmanga.info";
pub const UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
	AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 \
	Mobile/15E148 Safari/604.1";

pub fn urlencode(s: &str) -> String {
	let mut out = String::new();
	for byte in s.bytes() {
		match byte {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
				out.push(byte as char);
			}
			b' ' => out.push('+'),
			_ => {
				let hi = byte >> 4;
				let lo = byte & 0xf;
				out.push('%');
				out.push(
					char::from_digit(hi as u32, 16)
						.unwrap_or('0')
						.to_ascii_uppercase(),
				);
				out.push(
					char::from_digit(lo as u32, 16)
						.unwrap_or('0')
						.to_ascii_uppercase(),
				);
			}
		}
	}
	out
}

pub fn page_url(base: &str, page: i32) -> String {
	if page <= 1 {
		return base.to_string();
	}
	if let Some((path, query)) = base.split_once('?') {
		alloc::format!("{}/page/{}/?{}", path.trim_end_matches('/'), page, query)
	} else {
		alloc::format!("{}/page/{}/", base.trim_end_matches('/'), page)
	}
}

pub fn clean_title(raw: &str) -> String {
	raw.find(" [")
		.map_or_else(|| raw.trim().to_string(), |i| raw[..i].trim().to_string())
}

pub fn get_user_languages() -> Vec<String> {
	let mut slugs: Vec<String> = Vec::new();

	let langs = defaults_get::<Vec<String>>("languages")
		.or_else(|| defaults_get::<String>("languages").map(|s| alloc::vec![s]))
		.or_else(|| defaults_get::<String>("language").map(|s| alloc::vec![s]))
		.unwrap_or_default();

	for lang in langs {
		if let Some(slug) = map_lang_to_class(&lang) {
			let slug = slug.to_string();
			if !slugs.contains(&slug) {
				slugs.push(slug);
			}
		}
	}

	slugs
}

pub fn map_lang_to_class(lang: &str) -> Option<&'static str> {
	match lang.to_lowercase().trim() {
		"all" | "none" | "any" | "" => None,
		"en" | "english" => Some("english"),
		"ja" | "jp" | "japanese" => Some("jp"),
		"zh" | "cn" | "chinese" => Some("chinese"),
		"ko" | "kr" | "korean" => Some("korean"),
		"es" | "spanish" => Some("spanish"),
		"fr" | "french" => Some("french"),
		"de" | "german" => Some("german"),
		"it" | "italian" => Some("italian"),
		"pt" | "portuguese" => Some("portuguese"),
		_ => None,
	}
}

pub fn get(url: &str) -> aidoku::imports::error::Result<Document> {
	Ok(Request::get(url)?
		.header("User-Agent", UA)
		.header("Referer", BASE_URL)
		.html()?)
}
