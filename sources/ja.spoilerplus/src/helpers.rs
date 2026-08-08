use aidoku::{
	alloc::string::String,
	helpers::uri::{decode_uri, encode_uri},
	prelude::format,
};

pub const BASE_URL: &str = "https://spoilerplus.tv";

pub fn clean_title(title: String) -> String {
	let suffixes = [" Raw Free", " Raw free", " raw free"];
	for suffix in suffixes {
		if let Some(clean) = title.strip_suffix(suffix) {
			return clean.trim().into();
		}
	}
	title
}

// The listing markup holds relative hrefs while the pagination and navigation
// blocks hold absolute ones, and both have to collapse to the same key. Keys are
// kept decoded because the site percent-encodes series slugs but leaves chapter
// segments as raw utf-8, and `encode_uri` would turn the `%` of the mixed form
// into `%25`.
pub fn to_key(href: &str) -> Option<String> {
	let path = match href.strip_prefix(BASE_URL) {
		Some(path) => path,
		None if href.starts_with('/') => href,
		None => return None,
	};
	let decoded = decode_uri(path);
	(!decoded.is_empty()).then_some(decoded)
}

// A raw utf-8 path is answered with a 404, so keys are encoded on the way out.
pub fn url_for(key: &str) -> String {
	format!("{BASE_URL}{}", encode_uri(key))
}
