use aidoku::{
	HashMap, Result,
	alloc::{String, format},
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_get_map, defaults_set},
		net::Request,
	},
};

pub const LOGIN_KEY: &str = "login";
pub const MANUAL_COOKIE_KEY: &str = "manual_cookie";
const COOKIE_KEY: &str = "naver_cookies";

/// Helper to find cookie value ignoring case
fn get_cookie_ci<'a>(cookies: &'a HashMap<String, String>, target: &str) -> Option<&'a String> {
	if let Some(val) = cookies.get(target) {
		return Some(val);
	}
	cookies
		.iter()
		.find(|(k, _)| k.eq_ignore_ascii_case(target))
		.map(|(_, v)| v)
}

/// Save cookies captured from in-app Naver login
pub fn handle_login(cookies: HashMap<String, String>) -> Result<bool> {
	// Look for essential Naver session cookies (case-insensitive)
	let has_aut = get_cookie_ci(&cookies, "NID_AUT").is_some_and(|v| !v.trim().is_empty());
	let has_ses = get_cookie_ci(&cookies, "NID_SES").is_some_and(|v| !v.trim().is_empty());
	let has_neo = get_cookie_ci(&cookies, "NEO_SES").is_some_and(|v| !v.trim().is_empty());
	let has_jkl = get_cookie_ci(&cookies, "NID_JKL").is_some_and(|v| !v.trim().is_empty());
	let has_inf = get_cookie_ci(&cookies, "nid_inf").is_some_and(|v| !v.trim().is_empty());

	// If at least one primary session token is present, login is successful
	let is_authenticated = has_aut || has_ses || has_neo || has_jkl || has_inf;
	if !is_authenticated {
		return Ok(false);
	}

	// Format cookie string for outbound requests
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

	if !cookie_str.is_empty() {
		defaults_set(COOKIE_KEY, DefaultValue::String(cookie_str));
	}

	Ok(true)
}

#[allow(dead_code)]
pub fn is_logged_in() -> bool {
	// 1. Check manual cookie first
	if let Some(manual) = defaults_get::<String>(MANUAL_COOKIE_KEY)
		&& !manual.trim().is_empty()
		&& (manual.contains("NID_AUT") || manual.contains("NID_SES"))
	{
		return true;
	}

	// 2. Check cached cookie string
	if let Some(cookie_str) = defaults_get::<String>(COOKIE_KEY)
		&& !cookie_str.trim().is_empty()
		&& (cookie_str.contains("NID_AUT")
			|| cookie_str.contains("NID_SES")
			|| cookie_str.contains("NEO_SES"))
	{
		return true;
	}

	// 3. Check web login defaults map
	if let Some(map) = defaults_get_map(LOGIN_KEY) {
		let has_aut = get_cookie_ci(&map, "NID_AUT").is_some_and(|v| !v.trim().is_empty());
		let has_ses = get_cookie_ci(&map, "NID_SES").is_some_and(|v| !v.trim().is_empty());
		let has_neo = get_cookie_ci(&map, "NEO_SES").is_some_and(|v| !v.trim().is_empty());
		if has_aut || has_ses || has_neo {
			return true;
		}
	}

	false
}

/// Retrieve formatted Cookie header string
pub fn get_cookie_header() -> Option<String> {
	// 1. Manual cookie input has highest priority
	if let Some(manual) = defaults_get::<String>(MANUAL_COOKIE_KEY) {
		let trimmed = manual.trim();
		if !trimmed.is_empty() {
			return Some(String::from(trimmed));
		}
	}

	// 2. Cached formatted cookie string
	if let Some(cookie_str) = defaults_get::<String>(COOKIE_KEY).filter(|s| !s.trim().is_empty()) {
		return Some(cookie_str);
	}

	// 3. Reconstruct from defaults_get_map
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

/// Logout and clear saved cookies.
pub fn logout() {
	// Best-effort server-side logout
	if let Some(cookie) = get_cookie_header() {
		let _ = Request::get("https://nid.naver.com/nidlogin.logout")
			.map(|r| {
				r.header("Referer", "https://comic.naver.com/")
					.header("Cookie", &cookie)
			})
			.and_then(|r| r.send());
	}

	// Purge all stored credentials
	defaults_set(LOGIN_KEY, DefaultValue::Null);
	defaults_set(&format!("{LOGIN_KEY}.keys"), DefaultValue::Null);
	defaults_set(&format!("{LOGIN_KEY}.values"), DefaultValue::Null);
	defaults_set(COOKIE_KEY, DefaultValue::Null);
	defaults_set(MANUAL_COOKIE_KEY, DefaultValue::Null);
}
