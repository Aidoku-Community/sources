use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

use crate::helpers::{CONTENT_RATINGS, STATUS, TYPES};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";
const TYPES_KEY: &str = "types";
const STATUS_KEY: &str = "status";
const CONTENT_RATING_KEY: &str = "contentRating";

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn type_filters() -> Vec<&'static str> {
	known_values(
		defaults_get::<Vec<String>>(TYPES_KEY)
			.as_deref()
			.unwrap_or_default(),
		TYPES,
	)
}

pub fn status_filter() -> Vec<&'static str> {
	known_values(
		defaults_get::<Vec<String>>(STATUS_KEY)
			.as_deref()
			.unwrap_or_default(),
		STATUS,
	)
}

pub fn content_rating_filter() -> Vec<&'static str> {
	known_values(
		defaults_get::<Vec<String>>(CONTENT_RATING_KEY)
			.as_deref()
			.unwrap_or_default(),
		CONTENT_RATINGS,
	)
}

const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const HIDE_MTL_KEY: &str = "hideMtl";

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn hide_mtl() -> bool {
	defaults_get::<bool>(HIDE_MTL_KEY).unwrap_or(false)
}

fn known_values(values: &[String], known: &[&'static str]) -> Vec<&'static str> {
	values
		.iter()
		.filter_map(|value| {
			known
				.iter()
				.copied()
				.find(|known| value.eq_ignore_ascii_case(known))
		})
		.collect()
}
