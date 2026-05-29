use aidoku::imports::defaults::defaults_get;

const HIDE_NSFW_KEY: &str = "hideNSFW";
const DEDUPED_CHAPTER_KEY: &str = "dedupedChapter";

pub fn hide_nsfw() -> bool {
	defaults_get::<bool>(HIDE_NSFW_KEY).unwrap_or(true)
}

pub fn deduped_chapter() -> bool {
	defaults_get::<bool>(DEDUPED_CHAPTER_KEY).unwrap_or(false)
}
