use aidoku::{alloc::string::String, imports::defaults::defaults_get};

const BASE_URL_KEY: &str = "baseUrl";

pub fn get_base_url() -> String {
	defaults_get::<String>(BASE_URL_KEY).unwrap_or_default()
}
