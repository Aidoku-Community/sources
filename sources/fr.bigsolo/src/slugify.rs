use aidoku::alloc::{String, string::ToString};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

pub fn slugify(text: &str) -> String {
	// https://github.com/Bigherooooo/BigSolo-Site/blob/6b88af4c311b94730308fdb1ad9eaefe725dcd43/js/utils/domUtils.js#L30C1-L30C32

	// Remove `.json` suffix if present (case-insensitive)
	let mut text = text.trim_end().to_string();
	if text.to_lowercase().ends_with(".json") {
		text = text[..text.len() - 5].to_string(); // Remove last 5 characters
	}

	// Normalize the text (NFD separates accents from characters)
	let normalized: String = text.nfd().collect();

	// Remove diacritics (characters in the Unicode range \u0300-\u036f)
	let re_diacritics = Regex::new(r"[\u0300-\u036f]").unwrap();
	let without_diacritics = re_diacritics.replace_all(&normalized, "");

	// Convert to lowercase and trim whitespace
	let mut result = without_diacritics.to_lowercase().trim().to_string();

	// Replace all spaces (including ideographic spaces) with underscores
	let re_spaces = Regex::new(r"[\s\u{3000}]+").unwrap();
	result = re_spaces.replace_all(&result, "_").into_owned();

	// Remove all non-word characters except underscores and hyphens
	let re_invalid_chars = Regex::new(r"[^\w-]+").unwrap();
	result = re_invalid_chars.replace_all(&result, "").into_owned();

	// Replace multiple hyphens with a single underscore
	let re_multiple_hyphens = Regex::new(r"--+").unwrap();
	result = re_multiple_hyphens.replace_all(&result, "_").into_owned();

	result
}
