use crate::filters::GROUPS;
use aidoku::{
	alloc::{String, Vec, format},
	imports::defaults::defaults_get,
};

pub const CONTENT_TYPES: [&str; 6] = ["Manga", "Manwha", "Manhua", "OEL", "Other", "Novel"]; // "Manwha" is the site's spelling.
pub const CONTENT_RATINGS: [&str; 4] = ["Safe", "Suggestive", "Erotica", "Pornographic"];
const DEFAULT_RATINGS: [&str; 2] = ["Safe", "Suggestive"];

fn stored_list(key: &str, default: &[&str]) -> Vec<String> {
	defaults_get::<Vec<String>>(key)
		.unwrap_or_else(|| default.iter().map(|value| (*value).into()).collect())
}

pub fn content_types() -> Vec<String> {
	stored_list("contentTypes", &CONTENT_TYPES)
}

pub fn content_ratings() -> Vec<String> {
	stored_list("contentRatings", &DEFAULT_RATINGS)
}

pub fn adult_only() -> bool {
	defaults_get::<bool>("adultOnly").unwrap_or_default()
}

pub fn label_novels() -> bool {
	defaults_get::<bool>("labelNovels").unwrap_or(true)
}

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>("hiddenGenres").unwrap_or_default()
}

pub fn hidden_tags_key(group: &str) -> String {
	format!("hiddenTags.{group}")
}

pub fn hidden_tags() -> Vec<String> {
	GROUPS
		.iter()
		.filter_map(|group| defaults_get::<Vec<String>>(&hidden_tags_key(group)))
		.flatten()
		.collect()
}

// Ranking periods apply only to Discover and its listings.
pub fn timeframe(feed: &str) -> String {
	defaults_get::<String>(&format!("{feed}Timeframe")).unwrap_or_else(|| "weekly".into())
}

pub fn home_sections() -> Option<Vec<String>> {
	defaults_get::<Vec<String>>("homeSections")
}

pub enum Keep {
	HighestRated,
	Newest,
	MostPages,
}

pub fn deduplicate() -> Option<Keep> {
	if !defaults_get::<bool>("deduplicateChapters").unwrap_or_default() {
		return None;
	}
	Some(match defaults_get::<String>("deduplicateKeep").as_deref() {
		Some("newest") => Keep::Newest,
		Some("pages") => Keep::MostPages,
		_ => Keep::HighestRated,
	})
}

fn scanlator_names(key: &str) -> Vec<String> {
	defaults_get::<Vec<String>>(key)
		.unwrap_or_default()
		.iter()
		.flat_map(|entry| entry.split(','))
		.map(|name| name.trim().to_lowercase())
		.filter(|name| !name.is_empty())
		.collect()
}

pub fn excluded_scanlators() -> Vec<String> {
	scanlator_names("excludedScanlators")
}

pub fn preferred_scanlators() -> Vec<String> {
	scanlator_names("preferredScanlators")
}
