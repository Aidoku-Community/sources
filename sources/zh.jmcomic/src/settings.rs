use crate::models::AuthData;
use aidoku::{
	alloc::{String, Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};
use serde::{Serialize, de::DeserializeOwned};

const AUTH_DATA_KEY: &str = "authData";
const JUST_LOGGED_IN_KEY: &str = "justLoggedIn";
const BLOCKED_CONTENT_KEY: &str = "blockedMetadataKeywords";

fn get_str(key: &str) -> Option<String> {
	defaults_get::<String>(key).filter(|v| !v.is_empty())
}

fn set_str(key: &str, value: &str) {
	defaults_set(key, DefaultValue::String(value.into()));
}

fn get_json<T: DeserializeOwned>(key: &str) -> Option<T> {
	get_str(key).and_then(|v| serde_json::from_str(&v).ok())
}

fn set_json<T: Serialize>(key: &str, value: &T) {
	if let Ok(s) = serde_json::to_string(value) {
		set_str(key, &s);
	}
}

pub fn get_auth() -> Option<AuthData> {
	get_json(AUTH_DATA_KEY).filter(AuthData::is_valid)
}

pub fn set_auth(auth: &AuthData) {
	set_json(AUTH_DATA_KEY, auth);
}

pub fn clear_auth() {
	defaults_set(AUTH_DATA_KEY, DefaultValue::Null);
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
}

pub fn set_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
}

pub fn is_just_logged_in() -> bool {
	defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false)
}

pub fn clear_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
}

pub fn blocked_entries() -> Vec<String> {
	defaults_get::<Vec<String>>(BLOCKED_CONTENT_KEY)
		.unwrap_or_default()
		.into_iter()
		.map(|s| s.trim().to_lowercase())
		.filter(|s| !s.is_empty())
		.collect()
}
