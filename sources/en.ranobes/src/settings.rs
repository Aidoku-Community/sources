use aidoku::{
	alloc::{String, Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";
const HIDDEN_LANGUAGES_KEY: &str = "hiddenLanguages";

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn reset_hidden_genres() {
	defaults_set(HIDDEN_GENRES_KEY, DefaultValue::Null);
}

pub fn hidden_languages() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_LANGUAGES_KEY).unwrap_or_default()
}

pub fn reset_hidden_languages() {
	defaults_set(HIDDEN_LANGUAGES_KEY, DefaultValue::Null);
}
