use aidoku::{
	Result,
	alloc::String,
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	prelude::bail,
};

const BASE_URL_KEY: &str = "baseUrl";
const API_TOKEN_KEY: &str = "apiToken";
const USERNAME_KEY: &str = "credentials.username";
const PASSWORD_KEY: &str = "credentials.password";

pub fn get_base_url() -> Result<String> {
	let url = defaults_get::<String>(BASE_URL_KEY);
	match url {
		Some(url) if !url.is_empty() => Ok(url.trim_end_matches('/').into()),
		_ => bail!("Missing base URL: configure in settings"),
	}
}

pub fn get_api_token() -> Option<String> {
	defaults_get::<String>(API_TOKEN_KEY).filter(|s| !s.is_empty())
}

pub fn set_api_token(token: &str) {
	defaults_set(API_TOKEN_KEY, DefaultValue::String(token.into()));
}

pub fn get_credentials() -> Option<(String, String)> {
	let user = defaults_get::<String>(USERNAME_KEY)?;
	let pass = defaults_get::<String>(PASSWORD_KEY)?;
	if user.is_empty() {
		return None;
	}
	Some((user, pass))
}
