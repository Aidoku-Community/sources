use aidoku::{
	alloc::{String, string::ToString},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};
// === Storage Keys ===

// Authentication
const TOKEN_KEY: &str = "auth_token";
const USERNAME_KEY: &str = "username";
const PASSWORD_KEY: &str = "password";
const JUST_LOGGED_IN_KEY: &str = "justLoggedIn";

// Check-in
const AUTO_CHECKIN_KEY: &str = "autoCheckin";
const LAST_CHECKIN_KEY: &str = "lastCheckin";

// Enhanced Mode
const ENHANCED_MODE_KEY: &str = "enhancedMode";
const DEEP_SEARCH_KEY: &str = "deepSearch";
const MIN_ENHANCED_LEVEL: i32 = 1;

// Proxy
const USE_PROXY_KEY: &str = "useProxy";
const PROXY_URL_KEY: &str = "proxyUrl";

// Cache
const USER_CACHE_KEY: &str = "userCache";

// === Authentication ===

pub fn get_credentials() -> Option<(String, String)> {
	let username = defaults_get::<String>(USERNAME_KEY)?;
	let password = defaults_get::<String>(PASSWORD_KEY)?;
	if username.is_empty() || password.is_empty() {
		None
	} else {
		Some((username, password))
	}
}

pub fn set_credentials(username: &str, password: &str) {
	defaults_set(USERNAME_KEY, DefaultValue::String(username.to_string()));
	defaults_set(PASSWORD_KEY, DefaultValue::String(password.to_string()));
}

pub fn get_token() -> Option<String> {
	defaults_get::<String>(TOKEN_KEY).filter(|s: &String| !s.is_empty())
}

pub fn set_token(token: &str) {
	defaults_set(TOKEN_KEY, DefaultValue::String(token.to_string()));
}

pub fn get_current_token() -> Option<String> {
	if get_enhanced_mode() {
		get_token()
	} else {
		None
	}
}

pub fn clear_token() {
	defaults_set(TOKEN_KEY, DefaultValue::Null);
	defaults_set(USERNAME_KEY, DefaultValue::Null);
	defaults_set(PASSWORD_KEY, DefaultValue::Null);
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
	defaults_get::<bool>(AUTO_CHECKIN_KEY).unwrap_or(false) && get_token().is_some()
}

pub fn has_checkin_flag() -> bool {
	let last_day_str = defaults_get::<String>(LAST_CHECKIN_KEY).unwrap_or_default();
	let last_day = last_day_str.parse::<i64>().unwrap_or(-1);

	let current_time = aidoku::imports::std::current_date();
	let offset = 28800; // Beijing Time (UTC+8)
	let current_day = (current_time + offset) / 86400;

	last_day == current_day
}

pub fn set_last_checkin() {
	let now = aidoku::imports::std::current_date();
	let offset = 28800;
	let day_id = (now + offset) / 86400;
	defaults_set(LAST_CHECKIN_KEY, DefaultValue::String(day_id.to_string()));
}

pub fn clear_checkin_flag() {
	defaults_set(LAST_CHECKIN_KEY, DefaultValue::Null);
}

// === Enhanced Mode & Deep Search ===

/// Check if user meets minimum level requirement for enhanced features
pub fn user_meets_level_requirement() -> bool {
	if let Some(cache) = get_user_cache() {
		cache.level >= MIN_ENHANCED_LEVEL
	} else {
		false // No cache = require login to enable enhanced mode
	}
}

/// Enhanced mode requires: toggle ON + valid token + Lv.1+
pub fn get_enhanced_mode() -> bool {
	defaults_get::<bool>(ENHANCED_MODE_KEY).unwrap_or(false) 
		&& get_token().is_some()
		&& user_meets_level_requirement()
}

/// Deep Search: requires Enhanced Mode + toggle ON
pub fn deep_search_enabled() -> bool {
	get_enhanced_mode() && defaults_get::<bool>(DEEP_SEARCH_KEY).unwrap_or(false)
}

// === Proxy Mode ===

pub fn get_use_proxy() -> bool {
	defaults_get::<bool>(USE_PROXY_KEY).unwrap_or(false)
}

pub fn get_proxy_url() -> Option<String> {
	defaults_get::<String>(PROXY_URL_KEY)
		.filter(|url| {
			url.starts_with("https://")
				&& url.len() > 10
				&& !url.contains("your-worker") // Exclude placeholder
		})
		.map(|url| url.trim_end_matches('/').to_string()) // Normalize: remove trailing slash
}

// === User Info Cache ===

#[derive(aidoku::serde::Serialize, aidoku::serde::Deserialize, Clone, Default)]
pub struct UserCache {
	pub level: i32,
	pub is_sign: bool,
	pub timestamp: f64,
}


pub fn get_user_cache() -> Option<UserCache> {
	aidoku::imports::defaults::defaults_get::<UserCache>(USER_CACHE_KEY)
}

pub fn set_user_cache(level: i32, is_sign: bool) {
	let now = aidoku::imports::std::current_date();
	let cache = UserCache {
		level,
		is_sign,
		timestamp: now as f64,
	};
	aidoku::imports::defaults::defaults_set_data(USER_CACHE_KEY, &cache);
}

pub fn clear_user_cache() {
	defaults_set(USER_CACHE_KEY, DefaultValue::Null);
}

pub fn is_cache_stale() -> bool {
	if let Some(cache) = get_user_cache() {
		let now = aidoku::imports::std::current_date();
		// 6 hours = 21600 seconds
		return (now as f64) - cache.timestamp >= 21600.0;
	}
	true
}

// === State Reset ===

pub fn reset_dependent_settings() {
	defaults_set(AUTO_CHECKIN_KEY, DefaultValue::Null);
	defaults_set(ENHANCED_MODE_KEY, DefaultValue::Null);
	defaults_set(DEEP_SEARCH_KEY, DefaultValue::Null);
}
