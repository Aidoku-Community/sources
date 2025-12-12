use aidoku::{AidokuError, alloc::string::String, imports::defaults::defaults_get, prelude::bail};

const BASE_URL_KEY: &str = "baseUrl";
const USER_KEY: &str = "user";
const PASS_KEY: &str = "pass";

pub fn get_base_url() -> Result<String, AidokuError> {
	let base_url = defaults_get::<String>(BASE_URL_KEY);
	match base_url {
		Some(url) if !url.is_empty() => Ok(url),
		_ => bail!("Base Url not configured"),
	}
}

pub fn get_credentials() -> Option<(String, String)> {
	let user = defaults_get::<String>(USER_KEY)?;
	let pass = defaults_get::<String>(PASS_KEY)?;
	if user.is_empty() || pass.is_empty() {
		None
	} else {
		Some((user, pass))
	}
}
