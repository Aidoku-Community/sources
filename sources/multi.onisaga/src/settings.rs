use aidoku::{
	alloc::{String, Vec, vec},
	imports::defaults::defaults_get,
};

pub fn languages() -> Vec<String> {
	defaults_get::<Vec<String>>("languages").unwrap_or_else(|| vec!["en".into()])
}

pub fn adult() -> bool {
	defaults_get::<String>("contentRating").is_some_and(|rating| rating == "adult")
}

// Each choice adds to the one before, as on the site. There is no Content rating search filter
// to override this: OniSaga only shows Mature and Explicit titles after its 18+ confirmation,
// which this setting gives, so a filter could not reach them on its own.
pub fn content_ratings() -> Vec<String> {
	let count = match defaults_get::<String>("contentRating").as_deref() {
		Some("safe") => 1,
		Some("adult") => 4,
		_ => 2,
	};
	["safe", "suggestive", "mature", "explicit"][..count]
		.iter()
		.map(|&rating| rating.into())
		.collect()
}

// None while every origin is chosen, so titles without one still show.
pub fn origins() -> Option<Vec<String>> {
	defaults_get::<Vec<String>>("origins").filter(|origins| origins.len() < 7)
}

pub fn excluded_genres() -> Vec<String> {
	defaults_get::<Vec<String>>("excludedGenres").unwrap_or_default()
}

pub fn deduplicate_chapters() -> bool {
	defaults_get::<bool>("deduplicateChapters").unwrap_or(false)
}
