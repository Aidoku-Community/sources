use aidoku::{
	alloc::{String, Vec, vec},
	imports::defaults::defaults_get,
};

/// Pornographic titles are opt-in, as they are on the site.
const DEFAULT_RATINGS: &[&str] = &["safe", "suggestive", "erotica"];

/// The options under Types in settings.json.
const TYPE_COUNT: usize = 5;

fn stored_list(key: &str, default: &[&str]) -> Vec<String> {
	defaults_get::<Vec<String>>(key)
		.unwrap_or_else(|| default.iter().map(|value| (*value).into()).collect())
}

pub fn content_ratings() -> Vec<String> {
	stored_list("contentRatings", DEFAULT_RATINGS)
}

/// The types to browse, or none when every one is on, since the catalogue then has nothing
/// to narrow.
pub fn types() -> Vec<String> {
	match defaults_get::<Vec<String>>("types") {
		// The catalogue matches nothing for a type it doesn't know, which is what clearing
		// every type asks for.
		Some(types) if types.is_empty() => vec!["none".into()],
		Some(types) if types.len() < TYPE_COUNT => types,
		_ => Vec::new(),
	}
}

pub fn hidden_tags() -> Vec<String> {
	defaults_get::<Vec<String>>("hiddenTags").unwrap_or_default()
}

pub fn deduplicate_chapters() -> bool {
	defaults_get::<bool>("deduplicateChapters").unwrap_or_default()
}

pub fn show_upload_size() -> bool {
	defaults_get::<bool>("showUploadSize").unwrap_or_default()
}

/// The home sections to load, or every one when the setting was never stored.
pub fn home_sections() -> Option<Vec<String>> {
	defaults_get::<Vec<String>>("homeSections")
}
