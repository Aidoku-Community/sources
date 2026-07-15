use aidoku::{
	alloc::{String, Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn reset_hidden_genres() {
	defaults_set(HIDDEN_GENRES_KEY, DefaultValue::Null);
}
