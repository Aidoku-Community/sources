use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::defaults::defaults_get,
	prelude::*,
};

pub const ALL_TYPES: &[&str] = &["manga", "manhwa", "manhua", "other", "oel", "novel"];
pub const ALL_RATINGS: &[&str] = &["safe", "suggestive", "erotica", "pornographic"];

/// An unset key takes the declared default, but a stored empty list stays empty:
/// unchecking every box must not silently mean every box.
fn stored_list(key: &str, default: &[&str]) -> Vec<String> {
	defaults_get::<Vec<String>>(key)
		.unwrap_or_else(|| default.iter().map(|value| (*value).into()).collect())
}

/// Selected languages, in the underscore form the API expects.
pub fn get_languages() -> Result<Vec<String>> {
	defaults_get::<Vec<String>>("languages")
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
		.ok_or(error!("Unable to fetch languages"))
}

pub fn get_content_types() -> Vec<String> {
	stored_list("contentTypes", ALL_TYPES)
}

pub fn get_content_ratings() -> Vec<String> {
	stored_list("contentRatings", ALL_RATINGS)
}

/// Off by default: the deduplicated list drops alternate scanlator uploads.
pub fn deduplicate_chapters() -> bool {
	defaults_get::<bool>("deduplicateChapters").unwrap_or(false)
}

pub fn get_excluded_genres() -> Vec<String> {
	defaults_get::<Vec<String>>("excludedGenres").unwrap_or_default()
}

/// Inverse of [`get_languages`]: API code back to the BCP 47 form Aidoku uses.
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
