use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

use crate::helpers::{CONTENT_RATING_IDS, STATUS_IDS};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";
const STATUS_KEY: &str = "status";
const CONTENT_RATING_KEY: &str = "contentRating";

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn status_filter() -> Vec<String> {
	known_values(defaults_get::<Vec<String>>(STATUS_KEY), STATUS_IDS)
}

pub fn content_rating_filter() -> Vec<String> {
	known_values(
		defaults_get::<Vec<String>>(CONTENT_RATING_KEY),
		CONTENT_RATING_IDS,
	)
}

/// A saved value the site no longer offers is dropped rather than sent as a
/// filter the API cannot match.
fn known_values(stored: Option<Vec<String>>, known: &[&str]) -> Vec<String> {
	stored
		.unwrap_or_default()
		.into_iter()
		.filter_map(|value| {
			let value = value.to_ascii_lowercase();
			known.iter().any(|id| *id == value).then_some(value)
		})
		.collect()
}

const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const HIDE_MTL_KEY: &str = "hideMtl";

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn hide_mtl() -> bool {
	defaults_get::<bool>(HIDE_MTL_KEY).unwrap_or(false)
}
