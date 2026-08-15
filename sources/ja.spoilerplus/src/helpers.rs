use aidoku::{
	FilterValue,
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

// the listing holds relative hrefs and the navigation blocks absolute ones, and
// both have to collapse to the same key. keys stay decoded: the site mixes
// percent-encoded slugs with raw utf-8 chapter segments, and `encode_uri` would
// turn that `%` into `%25`
pub fn to_key(href: &str) -> Option<String> {
	let path = match href.strip_prefix(BASE_URL) {
		Some(path) => path,
		None if href.starts_with('/') => href,
		None => return None,
	};
	let decoded = decode_uri(path);
	(!decoded.is_empty()).then_some(decoded)
}

// raw utf-8 paths are answered with a 404, so keys are encoded on the way out
pub fn url_for(key: &str) -> String {
	format!("{BASE_URL}{}", encode_uri(key))
}

pub fn sort_index(filters: &[FilterValue]) -> i32 {
	filters
		.iter()
		.find_map(|filter| match filter {
			FilterValue::Sort { index, .. } => Some(*index),
			_ => None,
		})
		.unwrap_or(0)
}

pub fn read_window_number(data: &str, name: &str, fractional: bool) -> Option<String> {
	let after = &data[data.find(name)? + name.len()..];
	let after_eq = after[after.find('=')? + 1..].trim_start();
	let end = after_eq
		.find(|c: char| !c.is_ascii_digit() && !(fractional && c == '.'))
		.unwrap_or(after_eq.len());
	let number = after_eq[..end].trim();
	(!number.is_empty()).then(|| number.into())
}
