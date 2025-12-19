use aidoku::imports::defaults::defaults_get;

const HIDE_NSFW_KEY: &str = "hideNSFW";

pub fn hide_nsfw() -> bool {
	defaults_get::<bool>(HIDE_NSFW_KEY).unwrap_or(true)
}
