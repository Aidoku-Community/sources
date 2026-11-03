use aidoku::{
	alloc::{String, Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";
const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const MTL_KEY: &str = "mtl";

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn mtl_mode() -> &'static str {
	match defaults_get::<String>(MTL_KEY).as_deref() {
		Some("only") => "only",
		Some("hidden") => "hidden",
		_ => "all",
	}
}

pub fn reset_hidden_genres() {
	defaults_set(HIDDEN_GENRES_KEY, DefaultValue::Null);
}
