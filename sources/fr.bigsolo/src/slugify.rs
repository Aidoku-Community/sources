use aidoku::alloc::{String, string::ToString};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

pub fn slugify(text: &str) -> String {
	// https://github.com/Bigherooooo/BigSolo-Site/blob/adca4cbd860126ae08a2d654bb7556b3dbbca1b5/js/utils/domUtils.js#L30

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

	// Replace all spaces (including ideographic spaces) with hyphens
	let re_spaces = Regex::new(r"[\s_]+").unwrap();
	result = re_spaces.replace_all(&result, "-").into_owned();

	// Remove all non-word characters except underscores and hyphens
	let re_invalid_chars = Regex::new(r"[^\w-]+").unwrap();
	result = re_invalid_chars.replace_all(&result, "").into_owned();

	// Replace multiple hyphens with a single hyphen
	let re_multiple_hyphens = Regex::new(r"--+").unwrap();
	result = re_multiple_hyphens.replace_all(&result, "-").into_owned();

	result
}
