use aidoku::{
	alloc::{String, string::ToString},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const TOKEN_KEY: &str = "auth_token";
const JUST_LOGGED_IN_KEY: &str = "justLoggedIn";
const AUTO_CHECKIN_KEY: &str = "autoCheckin";
const LAST_CHECKIN_KEY: &str = "lastCheckin";
const ENHANCED_MODE_KEY: &str = "enhancedMode";

// === Authentication ===

pub fn get_token() -> Option<String> {
	defaults_get::<String>(TOKEN_KEY).filter(|s| !s.is_empty())
}

pub fn set_token(token: &str) {
	defaults_set(TOKEN_KEY, DefaultValue::String(token.to_string()));
}

pub fn clear_token() {
	defaults_set(TOKEN_KEY, DefaultValue::Null);
}

// === Login State Flag (for logout detection) ===

pub fn set_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
}

pub fn is_just_logged_in() -> bool {
	defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false)
}

pub fn clear_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
}

pub fn get_auto_checkin() -> bool {
	defaults_get::<bool>(AUTO_CHECKIN_KEY).unwrap_or(false)
}

pub fn has_checkin_flag() -> bool {
	defaults_get::<String>(LAST_CHECKIN_KEY)
		.filter(|s| !s.is_empty())
		.is_some()
}

pub fn set_last_checkin(date: &str) {
	defaults_set(LAST_CHECKIN_KEY, DefaultValue::String(date.into()));
}

pub fn clear_checkin_flag() {
	defaults_set(LAST_CHECKIN_KEY, DefaultValue::Null);
}

/// Returns true if Enhanced Mode is enabled AND the user is logged in.
pub fn get_enhanced_mode() -> bool {
	defaults_get::<bool>(ENHANCED_MODE_KEY).unwrap_or(false) && get_token().is_some()
}

// === Hidden Content Setting ===

const SHOW_HIDDEN_KEY: &str = "showHiddenContent";

pub fn show_hidden_content() -> bool {
	get_enhanced_mode() && defaults_get::<bool>(SHOW_HIDDEN_KEY).unwrap_or(false)
}

// === Hidden Content Cache ===

const HIDDEN_CACHE_KEY: &str = "hiddenCache";
const HIDDEN_CACHE_TIME_KEY: &str = "hiddenCacheTime";
const CACHE_EXPIRY_SECS: f64 = 3600.0; // 1 hour

pub fn get_hidden_cache() -> Option<String> {
	defaults_get::<String>(HIDDEN_CACHE_KEY).filter(|s| !s.is_empty())
}

pub fn set_hidden_cache(data: &str) {
	defaults_set(HIDDEN_CACHE_KEY, DefaultValue::String(data.into()));
	let now = aidoku::imports::std::current_date();
	defaults_set(HIDDEN_CACHE_TIME_KEY, DefaultValue::String(now.to_string()));
}

/// Check if hidden cache is still valid (1 hour expiry)
pub fn is_hidden_cache_valid() -> bool {
	if let Some(cache_time_str) = defaults_get::<String>(HIDDEN_CACHE_TIME_KEY)
		&& let Ok(cache_time) = cache_time_str.parse::<i64>()
	{
		let now = aidoku::imports::std::current_date();
		return (now - cache_time) < CACHE_EXPIRY_SECS as i64;
	}
	false
}

pub fn clear_hidden_cache() {
	defaults_set(HIDDEN_CACHE_KEY, DefaultValue::Null);
	defaults_set(HIDDEN_CACHE_TIME_KEY, DefaultValue::Null);
}
