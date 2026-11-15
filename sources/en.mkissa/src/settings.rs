use aidoku::{alloc::String, imports::defaults::defaults_get};

const IMAGE_QUALITY_KEY: &str = "imageQuality";
const SHOW_ADULT_KEY: &str = "showAdult";

pub fn image_quality() -> String {
	defaults_get::<String>(IMAGE_QUALITY_KEY).unwrap_or_else(|| "original".into())
}

pub fn show_adult() -> bool {
	defaults_get::<bool>(SHOW_ADULT_KEY).unwrap_or(false)
}
