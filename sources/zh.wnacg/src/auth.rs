//! Web login session.
//!
//! The site gates `users-check_login.html` behind a slider puzzle captcha, so
//! credentials cannot be posted directly; login must happen in the app's web
//! view. The login cookie is a host-only cookie of `www.wnacg.com`, the same
//! domain as the login page, so it is already among the cookies the app passes
//! to `handle_web_login`. The source stores it and attaches it to every
//! request.
//!
//! There is no server-side session: `users-logout.html` only expires the
//! cookie in the browser while the old value stays valid, so logging out just
//! means forgetting it here.

use aidoku::{
	HashMap,
	alloc::{String, Vec, string::ToString},
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		html::Document,
		std::current_date,
	},
	prelude::*,
};

use crate::net::{base_url, request_with_cookie};

/// `Cookie` header captured at login.
const COOKIE_KEY: &str = "auth_cookie";
/// Display name read from the account page, shown in the settings footer.
const ACCOUNT_KEY: &str = "auth_account";
/// Time of the latest successful capture. The `login` notification arrives
/// for both login and logout; a fresh timestamp tells them apart.
const LOGGED_IN_AT_KEY: &str = "auth_logged_in_at";

/// A `login` notification received within this many seconds of a capture is
/// treated as a login, not a logout (same window as zh.bookwalker and
/// zh.komiic2).
const LOGIN_WINDOW: i64 = 60;

/// Login cookies are named like `MPIC_bnS5`. The suffix looks like a per-site
/// random value, so only the prefix is matched; this is an assumption, not a
/// verified fact.
const LOGIN_COOKIE_PREFIX: &str = "MPIC_";

pub fn is_logged_in() -> bool {
	cookie_header().is_some()
}

/// Attached to every page request. `None` until the first successful capture.
pub fn cookie_header() -> Option<String> {
	defaults_get::<String>(COOKIE_KEY).filter(|header| !header.is_empty())
}

pub fn account() -> Option<String> {
	defaults_get::<String>(ACCOUNT_KEY).filter(|name| !name.is_empty())
}

/// Called by `handle_web_login` on every cookie change of the login page.
/// Returns whether the source is now logged in, which the app uses to decide
/// when to close the login page.
pub fn capture_web_login(cookies: &HashMap<String, String>) -> bool {
	let Some(header) = login_cookie_header(cookies) else {
		return false;
	};
	match verify(&header) {
		Some(name) => {
			defaults_set(COOKIE_KEY, DefaultValue::String(header));
			defaults_set(ACCOUNT_KEY, DefaultValue::String(name));
			defaults_set(
				LOGGED_IN_AT_KEY,
				DefaultValue::String(current_date().to_string()),
			);
			true
		}
		None => false,
	}
}

/// The app sends the same `login` notification for login and logout.
pub fn handle_login_notification() {
	let logged_in_at = defaults_get::<String>(LOGGED_IN_AT_KEY)
		.and_then(|value| value.parse::<i64>().ok())
		.unwrap_or(0);
	if current_date() - logged_in_at < LOGIN_WINDOW {
		defaults_set(LOGGED_IN_AT_KEY, DefaultValue::Null);
		return;
	}
	clear_all();
}

fn clear_all() {
	defaults_set(COOKIE_KEY, DefaultValue::Null);
	defaults_set(ACCOUNT_KEY, DefaultValue::Null);
	defaults_set(LOGGED_IN_AT_KEY, DefaultValue::Null);
}

fn login_cookie_header(cookies: &HashMap<String, String>) -> Option<String> {
	let mut pairs: Vec<String> = cookies
		.iter()
		.filter(|(name, value)| name.starts_with(LOGIN_COOKIE_PREFIX) && !value.is_empty())
		.map(|(name, value)| format!("{name}={value}"))
		.collect();
	pairs.sort();
	(!pairs.is_empty()).then(|| pairs.join("; "))
}

/// Reads the account page with `header`; `Some(display name)` means logged
/// in. The same URL serves the login form when logged out.
fn verify(header: &str) -> Option<String> {
	let url = format!("{}/users.html", base_url().ok()?);
	let response = request_with_cookie(&url, Some(header)).ok()?.send().ok()?;
	if !(200..300).contains(&response.status_code()) {
		return None;
	}
	account_name(&response.get_html().ok()?)
}

/// The desktop account page only links to `users-logout.html` when logged in;
/// the display name sits next to the avatar in `.top_utab.ui`. The preceding
/// `.top_utab` is the notification menu whose 關注更新 also links to
/// `/users.html`. An empty name still counts as logged in; the footer just
/// shows no name.
fn account_name(html: &Document) -> Option<String> {
	html.select_first("a[href*=users-logout]")?;
	Some(
		html.select_first(".top_utab.ui > a[href$=\"users.html\"]")
			.and_then(|el| el.text())
			.map(|text| text.trim().to_string())
			.unwrap_or_default(),
	)
}
