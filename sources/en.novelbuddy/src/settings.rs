use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";
const STATUS_KEY: &str = "status";
const CONTENT_RATING_KEY: &str = "contentRating";

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn status_filter() -> Vec<String> {
	defaults_get::<Vec<String>>(STATUS_KEY).unwrap_or_default()
}

pub fn content_rating_filter() -> Vec<String> {
	defaults_get::<Vec<String>>(CONTENT_RATING_KEY).unwrap_or_default()
}

const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const HIDE_MTL_KEY: &str = "hideMtl";

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn hide_mtl() -> bool {
	defaults_get::<bool>(HIDE_MTL_KEY).unwrap_or(false)
}
