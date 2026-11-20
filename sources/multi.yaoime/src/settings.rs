use crate::tags::GROUPS;
use aidoku::{
	alloc::{String, Vec, format, vec},
	imports::defaults::defaults_get,
};

pub const CONTENT_RATINGS: [&str; 4] = ["safe", "suggestive", "erotica", "pornographic"];

pub fn languages() -> Vec<String> {
	defaults_get::<Vec<String>>("languages").unwrap_or_else(|| vec!["en".into()])
}

pub fn excluded_types() -> Vec<String> {
	defaults_get::<Vec<String>>("excludedTypes").unwrap_or_default()
}

pub fn content_ratings() -> Vec<String> {
	defaults_get::<Vec<String>>("contentRatings").unwrap_or_else(|| {
		CONTENT_RATINGS
			.iter()
			.map(|rating| (*rating).into())
			.collect()
	})
}

pub fn excluded_tags() -> Vec<String> {
	let groups = GROUPS
		.iter()
		.filter_map(|(group, _)| defaults_get::<Vec<String>>(&format!("excludedTags.{group}")));
	defaults_get::<Vec<String>>("excludedGenres")
		.into_iter()
		.chain(groups)
		.flatten()
		.collect()
}

pub fn title_preference() -> String {
	defaults_get::<String>("titlePreference").unwrap_or_else(|| "site".into())
}

pub fn official_only() -> bool {
	defaults_get::<bool>("officialOnly").unwrap_or_default()
}

pub fn clean_chapters() -> bool {
	defaults_get::<bool>("cleanChapters").unwrap_or_default()
}

pub fn home_sections() -> Option<Vec<String>> {
	defaults_get::<Vec<String>>("homeSections")
}
