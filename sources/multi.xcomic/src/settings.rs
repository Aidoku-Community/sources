use aidoku::{
	alloc::{String, Vec, vec},
	imports::defaults::defaults_get,
};

pub const CONTENT_TYPES: &[&str] = &[
	"cartoon", "imageset", "manga", "manhua", "manhwa", "oel", "other", "western",
];
const CONTENT_RATINGS: &[&str] = &["safe", "suggestive", "erotica", "pornographic"];

/// An unset key takes the declared default, but a stored empty list stays empty:
/// unchecking every box must not silently mean every box.
fn stored_list(key: &str, default: &[&str]) -> Vec<String> {
	defaults_get::<Vec<String>>(key)
		.unwrap_or_else(|| default.iter().map(|value| (*value).into()).collect())
}

pub fn content_types() -> Vec<String> {
	stored_list("contentTypes", CONTENT_TYPES)
}

pub fn content_ratings() -> Vec<String> {
	stored_list("contentRatings", CONTENT_RATINGS)
}

pub fn excluded_genres() -> Vec<String> {
	defaults_get::<Vec<String>>("excludedGenres").unwrap_or_default()
}

pub fn show_source_in_title() -> bool {
	defaults_get::<bool>("showSourceInTitle").unwrap_or(true)
}

/// On by default: a title with hundreds of sources pages its full list far enough
/// to draw a 429, which loses the list entirely.
pub fn deduplicate_chapters() -> bool {
	defaults_get::<bool>("deduplicateChapters").unwrap_or(true)
}

/// Selected languages, in the underscore form the API expects. Falls back to English
/// when nothing is selected.
pub fn languages() -> Vec<String> {
	defaults_get::<Vec<String>>("languages")
		.filter(|languages| !languages.is_empty())
		.map(|languages| {
			languages
				.into_iter()
				.map(|language| match language.as_str() {
					"pt-BR" => "pt_br".into(),
					"es-419" => "es_419".into(),
					_ => language,
				})
				.collect()
		})
		.unwrap_or_else(|| vec!["en".into()])
}

/// Inverse of [`languages`]: API code back to the BCP 47 form Aidoku uses.
pub fn normalize_language(language: &str) -> Option<String> {
	let language = language.trim();
	if language.is_empty() {
		return None;
	}
	Some(match language {
		"pt_br" => "pt-BR".into(),
		"es_419" => "es-419".into(),
		_ => language.replace('_', "-"),
	})
}
