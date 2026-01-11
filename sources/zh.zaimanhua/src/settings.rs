use aidoku::{
	alloc::{String, string::ToString},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const TOKEN_KEY: &str = "auth_token";
const JUST_LOGGED_IN_KEY: &str = "justLoggedIn";
const AUTO_CHECKIN_KEY: &str = "autoCheckin";
const LAST_CHECKIN_KEY: &str = "lastCheckin";
const ENHANCED_MODE_KEY: &str = "enhancedMode";
const SHOW_HIDDEN_KEY: &str = "showHiddenContent";

// === Authentication ===

pub fn get_token() -> Option<String> {
	defaults_get::<String>(TOKEN_KEY).filter(|s: &String| !s.is_empty())
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

// === Daily Check-in Logic ===

pub fn get_auto_checkin() -> bool {
	defaults_get::<bool>(AUTO_CHECKIN_KEY).unwrap_or(false)
}

pub fn has_checkin_flag() -> bool {
	let last_time_str = defaults_get::<String>(LAST_CHECKIN_KEY).unwrap_or_default();
	let last_time = last_time_str.parse::<i64>().unwrap_or(0);
	let current_time = aidoku::imports::std::current_date(); 
	let offset = 28800;
	let last_day = (last_time + offset) / 86400;
	let current_day = (current_time + offset) / 86400;
	last_day == current_day
}

pub fn set_last_checkin() {
	let now = aidoku::imports::std::current_date();
	defaults_set(LAST_CHECKIN_KEY, DefaultValue::String(now.to_string()));
}

pub fn clear_checkin_flag() {
	defaults_set(LAST_CHECKIN_KEY, DefaultValue::Null);
}

// === Enhanced Mode & Hidden Content ===

pub fn get_enhanced_mode() -> bool {
	defaults_get::<bool>(ENHANCED_MODE_KEY).unwrap_or(false) && get_token().is_some()
}

pub fn show_hidden_content() -> bool {
	get_enhanced_mode() && defaults_get::<bool>(SHOW_HIDDEN_KEY).unwrap_or(false)
}