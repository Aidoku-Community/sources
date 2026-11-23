use crate::helpers::narrowed;
use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

pub fn content_rating(default: &str) -> String {
	defaults_get::<String>("contentRating").unwrap_or_else(|| default.into())
}

pub fn types(count: usize) -> Vec<String> {
	defaults_get::<Vec<String>>("types")
		.map(|types| narrowed(types, count))
		.unwrap_or_default()
}

pub fn excluded_genres() -> Vec<String> {
	defaults_get::<Vec<String>>("excludedGenres").unwrap_or_default()
}
