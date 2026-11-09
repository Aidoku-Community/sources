use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";
const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const HIDE_MTL_KEY: &str = "hideMtl";
const STATUS_KEY: &str = "status";

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn hide_mtl() -> bool {
	defaults_get::<bool>(HIDE_MTL_KEY).unwrap_or(false)
}

pub fn status_filter() -> Vec<String> {
	defaults_get::<Vec<String>>(STATUS_KEY).unwrap_or_default()
}
