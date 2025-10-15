use aidoku::{AidokuError, alloc::string::String, imports::defaults::defaults_get, prelude::bail};

const BASE_URL_KEY: &str = "baseUrl";
const API_KEY_KEY: &str = "apiKey";

pub fn get_base_url() -> Result<String, AidokuError> {
	let base_url = defaults_get::<String>(BASE_URL_KEY);
	match base_url {
		Some(url) if !url.is_empty() => Ok(url),
		_ => bail!("Missing base URL: configure in settings"),
	}
}

pub fn get_api_key() -> String {
	defaults_get::<String>(API_KEY_KEY).unwrap_or_default()
}
