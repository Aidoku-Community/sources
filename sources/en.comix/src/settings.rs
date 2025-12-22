use aidoku::{alloc::string::String, imports::defaults::defaults_get};

const HIDE_NSFW_KEY: &str = "hideNSFW";
const THUMBNAIL_QUALITY_KEY: &str = "thumbnailQuality";
const DEDUPED_CHAPTER_KEY: &str = "dedupedChapter";

pub fn hide_nsfw() -> bool {
	defaults_get::<bool>(HIDE_NSFW_KEY).unwrap_or(true)
}

pub fn get_image_quality() -> String {
	defaults_get::<String>(THUMBNAIL_QUALITY_KEY).unwrap_or_default()
}

pub fn get_dedupchapter() -> bool {
	defaults_get::<bool>(DEDUPED_CHAPTER_KEY).unwrap_or(false)
}
