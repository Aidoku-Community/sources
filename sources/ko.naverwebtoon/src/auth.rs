use aidoku::{
	HashMap, Result,
	alloc::{String, format},
	imports::defaults::{DefaultValue, defaults_get, defaults_get_map, defaults_set},
};

const LOGIN_KEY: &str = "login";
const COOKIE_KEY: &str = "naver_cookies";

/// Save cookies captured from in-app Naver login
pub fn handle_login(cookies: HashMap<String, String>) -> Result<bool> {
	// Only finish login if both NID_AUT and NID_SES session cookies are present and non-empty.
	// Returning false keeps the webview open for the user to complete login.
	let is_authenticated = ["NID_AUT", "NID_SES"].iter().all(|name| {
		cookies
			.get(*name)
			.is_some_and(|value| !value.trim().is_empty())
	});
	if !is_authenticated {
		return Ok(false);
	}

	// Aidoku automatically saves cookies to LOGIN_KEY as a map when returning Ok(true),
	// but we also cache the formatted cookie string for fast access without reconstructing the map on every request.
	let mut cookie_str = String::new();
	for (name, value) in cookies.iter() {
		if value.trim().is_empty() {
			continue;
		}
		if !cookie_str.is_empty() {
			cookie_str.push_str("; ");
		}
		cookie_str.push_str(name);
		cookie_str.push('=');
		cookie_str.push_str(value);
	}
	defaults_set(COOKIE_KEY, DefaultValue::String(cookie_str));
	Ok(true)
}

/// Check if user has authenticated Naver cookies
pub fn is_logged_in() -> bool {
	if let Some(map) = defaults_get_map(LOGIN_KEY) {
		let has_aut = map.get("NID_AUT").is_some_and(|v| !v.trim().is_empty());
		let has_ses = map.get("NID_SES").is_some_and(|v| !v.trim().is_empty());
		if has_aut && has_ses {
			return true;
		}
	}
	false
}

/// Retrieve formatted Cookie header string (prioritizes fast cached string, falls back to map)
pub fn get_cookie_header() -> Option<String> {
	if !is_logged_in() {
		return None;
	}
	if let Some(cookie_str) = defaults_get::<String>(COOKIE_KEY).filter(|s| !s.trim().is_empty()) {
		return Some(cookie_str);
	}
	if let Some(map) = defaults_get_map(LOGIN_KEY) {
		let mut header = String::new();
		for (k, v) in map.iter() {
			if v.trim().is_empty() {
				continue;
			}
			if !header.is_empty() {
				header.push_str("; ");
			}
			header.push_str(k);
			header.push('=');
			header.push_str(v);
		}
		if !header.is_empty() {
			return Some(header);
		}
	}
	None
}

/// Logout and clear saved cookies
pub fn logout() {
	defaults_set(LOGIN_KEY, DefaultValue::Null);
	defaults_set(&format!("{LOGIN_KEY}.keys"), DefaultValue::Null);
	defaults_set(&format!("{LOGIN_KEY}.values"), DefaultValue::Null);
	defaults_set(COOKIE_KEY, DefaultValue::Null);
}
