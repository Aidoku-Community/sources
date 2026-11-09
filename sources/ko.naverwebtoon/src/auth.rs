use aidoku::{
	HashMap, Result,
	alloc::{String, Vec, format},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const AUTH_KEY: &str = "auth";

/// Helper to find cookie value ignoring case
fn get_cookie<'a>(cookies: &'a HashMap<String, String>, name: &str) -> Option<&'a String> {
	cookies.get(name).or_else(|| {
		cookies
			.iter()
			.find(|(k, _)| k.eq_ignore_ascii_case(name))
			.map(|(_, v)| v)
	})
}

/// Save cookies captured from in-app Naver login
pub fn handle_login(cookies: HashMap<String, String>) -> Result<bool> {
	let has_aut = get_cookie(&cookies, "NID_AUT").is_some_and(|v| !v.trim().is_empty());
	let has_ses = get_cookie(&cookies, "NID_SES").is_some_and(|v| !v.trim().is_empty());

	if !has_aut || !has_ses {
		return Ok(false);
	}

	let mut cookie_parts = Vec::new();
	for (name, value) in &cookies {
		if !value.trim().is_empty() {
			cookie_parts.push(format!("{name}={value}"));
		}
	}
	let cookie_str = cookie_parts.join("; ");

	if !cookie_str.is_empty() {
		defaults_set(AUTH_KEY, DefaultValue::String(cookie_str));
	}

	Ok(true)
}

#[allow(dead_code)]
pub fn is_logged_in() -> bool {
	defaults_get::<String>(AUTH_KEY).is_some()
}

pub fn get_cookie_header() -> Option<String> {
	defaults_get::<String>(AUTH_KEY).filter(|s| !s.trim().is_empty())
}

pub fn logout() {
	defaults_set(AUTH_KEY, DefaultValue::Null);
}
