use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

pub fn base_url() -> String {
	defaults_get::<String>("url").unwrap_or_else(|| "https://mangavyvy.com".into())
}

pub fn excluded_genres() -> Vec<String> {
	defaults_get::<Vec<String>>("excludedGenres").unwrap_or_default()
}
