use aidoku::{
	alloc::{String, Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";
const HIDDEN_LANGUAGES_KEY: &str = "hiddenLanguages";
const CF_COOKIE_KEY: &str = "cfCookie";

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

/// Persisted across calls (and app restarts), matching multi.ehentai's
/// pattern for its own Cloudflare-adjacent session cookie — a cookie
/// that already proved itself once is likely to keep working for a
/// while, so there's no reason to start "cold" on every fetch.
pub fn cf_cookie() -> Option<String> {
	defaults_get::<String>(CF_COOKIE_KEY)
}

pub fn set_cf_cookie(value: &str) {
	defaults_set(CF_COOKIE_KEY, DefaultValue::String(value.into()));
}
