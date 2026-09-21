use aidoku::{
	HashMap, Result,
	alloc::String,
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const COOKIE_KEY: &str = "naver_cookies";

/// Save cookies captured from in-app Naver login
pub fn handle_login(cookies: HashMap<String, String>) -> Result<bool> {
	// Only finish login if authenticated session cookies are present and non-empty.
	// Returning false keeps the webview open for the user to complete login.
	let is_authenticated = ["NID_AUT", "NID_SES"]
		.iter()
		.any(|name| cookies.get(*name).is_some_and(|value| !value.trim().is_empty()));
	if !is_authenticated {
		return Ok(false);
	}
	let mut cookie_str = String::new();
	let mut first = true;
	for (name, value) in cookies.iter() {
		if value.trim().is_empty() {
			continue;
		}
		if !first {
			cookie_str.push_str("; ");
		}
		cookie_str.push_str(name);
		cookie_str.push('=');
		cookie_str.push_str(value);
		first = false;
	}
	defaults_set(COOKIE_KEY, DefaultValue::String(cookie_str));
	Ok(true)
}

/// Check if user has authenticated Naver cookies
pub fn is_logged_in() -> bool {
	if let Some(cookie_str) = defaults_get::<String>(COOKIE_KEY) {
		cookie_str.split(';').any(|part| {
			let mut split = part.splitn(2, '=');
			let name = split.next().unwrap_or("").trim();
			let val = split.next().unwrap_or("").trim();
			(name == "NID_AUT" || name == "NID_SES") && !val.is_empty()
		})
	} else {
		false
	}
}

/// Retrieve formatted Cookie header string
pub fn get_cookie_header() -> Option<String> {
	defaults_get::<String>(COOKIE_KEY).filter(|s| !s.trim().is_empty())
}

/// Logout and clear saved cookies
pub fn logout() {
	defaults_set(COOKIE_KEY, DefaultValue::Null);
}
