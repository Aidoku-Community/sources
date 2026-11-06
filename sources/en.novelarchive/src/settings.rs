use aidoku::{alloc::String, imports::defaults::defaults_get};

const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const STATUS_KEY: &str = "status";

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn status() -> String {
	defaults_get::<String>(STATUS_KEY).unwrap_or_else(|| "all".into())
}
