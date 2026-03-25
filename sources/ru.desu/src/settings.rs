use aidoku::imports::defaults::defaults_get;
use alloc::string::{String, ToString};

const ENGLISH_TITLE_KEY: &str = "englishTitles";
const DOMAIN_KEY: &str = "domain";

pub fn eng_title() -> bool {
	defaults_get::<bool>(ENGLISH_TITLE_KEY).unwrap_or(false)
}

pub fn domain() -> String {
	defaults_get::<String>(DOMAIN_KEY).unwrap_or("desu.uno".to_string())
}
