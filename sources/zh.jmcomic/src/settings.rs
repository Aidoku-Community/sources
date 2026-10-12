use crate::models::{AuthData, BlockState};
use aidoku::{
	alloc::{String, Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_get_json, defaults_set},
};

const AUTH_DATA_KEY: &str = "authData";
const USERNAME_KEY: &str = "login.username";
const BLOCKED_CONTENT_KEY: &str = "blockedMetadataKeywords";

pub fn get_auth() -> Option<AuthData> {
	defaults_get_json(AUTH_DATA_KEY)
		.ok()
		.filter(AuthData::is_valid)
}

pub fn set_auth(auth: &AuthData) {
	if let Ok(json) = serde_json::to_string(auth) {
		defaults_set(AUTH_DATA_KEY, DefaultValue::String(json));
	}
}

pub fn clear_auth() {
	defaults_set(AUTH_DATA_KEY, DefaultValue::Null);
}

pub fn is_logged_in() -> bool {
	defaults_get::<String>(USERNAME_KEY).is_some_and(|username| !username.is_empty())
}

pub fn block_state() -> BlockState {
	BlockState::new(
		defaults_get::<Vec<String>>(BLOCKED_CONTENT_KEY)
			.unwrap_or_default()
			.into_iter()
			.map(|s| s.trim().to_lowercase())
			.filter(|s| !s.is_empty()),
	)
}
