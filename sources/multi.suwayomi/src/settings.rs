use aidoku::{
	Result,
	alloc::String,
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	prelude::*,
};

const BASE_URL_KEY: &str = "baseUrl";
const AUTH_MODE_KEY: &str = "authMode";
const USERNAME_KEY: &str = "credentials.username";
const PASSWORD_KEY: &str = "credentials.password";
const ACCESS_TOKEN_KEY: &str = "credentials.accessToken";
const REFRESH_TOKEN_KEY: &str = "credentials.refreshToken";

pub fn get_base_url() -> Result<String> {
	let url: String = defaults_get::<String>(BASE_URL_KEY).ok_or(error!("Missing baseUrl"))?;
	Ok(url.trim_end_matches('/').into())
}

pub fn get_auth_mode() -> String {
	match defaults_get::<String>(AUTH_MODE_KEY) {
		Some(v) if !v.is_empty() => v,
		_ => "auto".into(),
	}
}

pub fn get_credentials() -> Option<(String, String)> {
	let user: String = defaults_get::<String>(USERNAME_KEY)?;
	let pass: String = defaults_get::<String>(PASSWORD_KEY)?;

	if user.is_empty() {
		return None;
	}
	Some((user, pass))
}

pub fn get_access_token() -> Option<String> {
	defaults_get::<String>(ACCESS_TOKEN_KEY)
}

pub fn get_refresh_token() -> Option<String> {
	defaults_get::<String>(REFRESH_TOKEN_KEY)
}

pub fn set_tokens(access: &str, refresh: &str) {
	defaults_set(ACCESS_TOKEN_KEY, DefaultValue::String(access.into()));
	defaults_set(REFRESH_TOKEN_KEY, DefaultValue::String(refresh.into()));
}

pub fn set_access_token(access: &str) {
	defaults_set(ACCESS_TOKEN_KEY, DefaultValue::String(access.into()));
}

pub fn clear_tokens() {
	defaults_set(ACCESS_TOKEN_KEY, DefaultValue::Null);
	defaults_set(REFRESH_TOKEN_KEY, DefaultValue::Null);
}
