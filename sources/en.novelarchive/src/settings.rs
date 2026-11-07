use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

// Rated 17+, so adult novels are hidden by default.
pub fn hide_adult() -> bool {
	defaults_get::<bool>("hideAdult").unwrap_or(true)
}

// Dynamic settings read as unset until the settings page first opens.
pub fn hidden_genres() -> Vec<String> {
	defaults_get("hiddenGenres").unwrap_or_default()
}
