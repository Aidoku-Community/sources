use aidoku::{
	alloc::string::{String, ToString},
	helpers::uri::{decode_uri, encode_uri},
	prelude::format,
};

pub const BASE_URL: &str = "https://spoilerplus.tv";

/// Strip trailing "Raw Free" and similar suffixes from a title.
pub fn clean_title(title: String) -> String {
	let suffixes = [" Raw Free", " Raw free", " raw free"];
	for suffix in suffixes {
		if let Some(clean) = title.strip_suffix(suffix) {
			return clean.trim().into();
		}
	}
	title
}

/// Extract chapter number from text like 第N話 -> N
pub fn extract_ch_number(s: &str) -> Option<f32> {
	let dai = '第';
	let wa = '話';

	let start = s.find(dai)? + dai.len_utf8();
	let end = s[start..].find(wa)? + start;

	let num_str = &s[start..end];
	num_str.parse().ok()
}

/// Turn an href into a site-relative key.
///
/// Both forms of href have to collapse to the same key or the same series would
/// be stored twice: the listing markup holds relative hrefs while the pagination
/// and navigation blocks hold absolute ones. Keys are also stored decoded,
/// because the site percent-encodes series slugs but leaves chapter segments as
/// raw utf-8, and only one of the two forms can round-trip through [`url_for`].
pub fn to_key(href: &str) -> Option<String> {
	let path = match href.strip_prefix(BASE_URL) {
		Some(path) => path,
		// an href that is neither absolute for this site nor site-relative points
		// somewhere else entirely
		None if href.starts_with('/') => href,
		None => return None,
	};
	let decoded = decode_uri(path);
	(!decoded.is_empty()).then_some(decoded)
}

/// Build the url a key points at.
///
/// The site rejects raw utf-8 paths with a 404, so keys are encoded on the way
/// out. `encode_uri` also escapes `%`, which is why keys are kept decoded rather
/// than in the mixed form the markup uses.
pub fn url_for(key: &str) -> String {
	format!("{BASE_URL}{}", encode_uri(key))
}

/// Join a site-relative path into an absolute url, leaving absolute ones alone.
pub fn absolute_url(src: &str) -> String {
	if src.starts_with("http") {
		src.to_string()
	} else {
		let mut url = String::from(BASE_URL);
		if !src.starts_with('/') {
			url.push('/');
		}
		url.push_str(src);
		url
	}
}
