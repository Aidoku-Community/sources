use aidoku::{
	alloc::{String, Vec, vec},
	imports::defaults::defaults_get,
};

pub fn languages() -> Vec<String> {
	defaults_get::<Vec<String>>("languages").unwrap_or_else(|| vec!["en".into()])
}

pub fn adult_mode() -> &'static str {
	let enabled = |key| defaults_get::<bool>(key).unwrap_or_default();
	match (enabled("showAdult"), enabled("adultOnly")) {
		(false, _) => "no_18",
		(true, false) => "all",
		(true, true) => "only_18",
	}
}

pub fn content_type() -> String {
	defaults_get::<String>("type")
		.filter(|kind| kind != "any")
		.unwrap_or_default()
}

pub fn excluded_tags() -> Vec<String> {
	[
		"excludedGenres",
		"excludedThemes",
		"excludedFormats",
		"excludedContent",
	]
	.iter()
	.filter_map(|key| defaults_get::<Vec<String>>(key))
	.flatten()
	.collect()
}

pub fn excluded_scanlators() -> Vec<String> {
	defaults_get::<Vec<String>>("excludedScanlators")
		.unwrap_or_default()
		.into_iter()
		.map(|name| name.trim().to_lowercase())
		.collect()
}

pub fn deduplicate_chapters() -> bool {
	defaults_get::<bool>("deduplicateChapters").unwrap_or_default()
}
