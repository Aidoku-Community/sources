use aidoku::{AidokuError, alloc::string::String, imports::defaults::defaults_get, prelude::bail};

const BASE_URL_KEY: &str = "baseUrl";

pub fn get_base_url() -> Result<String, AidokuError> {
	let base_url = defaults_get::<String>(BASE_URL_KEY);
	match base_url {
		Some(url) if !url.is_empty() => Ok(url),
		_ => bail!("Base Url not configured"),
	}
}
