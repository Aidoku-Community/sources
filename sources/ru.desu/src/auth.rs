use crate::settings::base_url;
use aidoku::{
	HashMap, Result,
	alloc::{String, Vec, format, string::ToString},
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		html::Document,
		net::Request,
	},
	prelude::*,
};

const COOKIE_KEY: &str = "login.cookie";
const LOGGED_IN_KEY: &str = "login.ok";
const JUST_LOGGED_IN_KEY: &str = "login.just";
const USERNAME_KEY: &str = "login.username";
const STORED_USERNAME_KEY: &str = "desu.username";

/// XenForo session cookies required for authenticated Desu requests.
const XF_COOKIE_NAMES: &[&str] = &["xf_user", "xf_session"];

/// Whether a Desu session is available (flag or `xf_user` cookie).
pub fn is_logged_in() -> bool {
	defaults_get::<bool>(LOGGED_IN_KEY).unwrap_or(false)
		|| defaults_get::<String>(COOKIE_KEY).is_some_and(|c| c.contains("xf_user="))
}

/// Username shown in settings after a successful web login.
pub fn stored_username() -> Option<String> {
	defaults_get::<String>(STORED_USERNAME_KEY)
		.or_else(|| defaults_get::<String>(USERNAME_KEY))
		.filter(|s| !s.is_empty())
}

fn store_username(username: &str) {
	let trimmed = username.trim();
	if !trimmed.is_empty() {
		defaults_set(STORED_USERNAME_KEY, DefaultValue::String(trimmed.into()));
	}
}

fn set_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
}

/// Consumes the one-shot “login just succeeded” flag used by the login notification.
pub fn take_just_logged_in() -> bool {
	let flag = defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false);
	if flag {
		defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
	}
	flag
}

/// Clears the stored XenForo cookie header and account metadata.
pub fn logout() {
	defaults_set(COOKIE_KEY, DefaultValue::Null);
	defaults_set(LOGGED_IN_KEY, DefaultValue::Null);
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
	defaults_set(STORED_USERNAME_KEY, DefaultValue::Null);
}

fn store_cookie_header(header: &str) {
	defaults_set(COOKIE_KEY, DefaultValue::String(header.into()));
}

fn set_logged_in(value: bool) {
	if value {
		defaults_set(LOGGED_IN_KEY, DefaultValue::Bool(true));
	} else {
		defaults_set(LOGGED_IN_KEY, DefaultValue::Null);
	}
}

/// Builds `Cookie` header from WebView cookies (`xf_user` / `xf_session` only).
fn cookie_header_from_map(cookies: &HashMap<String, String>) -> Option<String> {
	let mut parts = Vec::new();
	for name in XF_COOKIE_NAMES {
		let value = cookies.get(*name).or_else(|| {
			cookies
				.iter()
				.find(|(k, _)| k.eq_ignore_ascii_case(name))
				.map(|(_, v)| v)
		});
		if let Some(value) = value.filter(|v| !v.is_empty()) {
			parts.push(format!("{name}={value}"));
		}
	}
	if parts.iter().any(|p| p.starts_with("xf_user=")) {
		Some(parts.join("; "))
	} else {
		None
	}
}

/// Attaches the stored Desu session cookie to outbound requests.
pub trait AuthedRequest {
	fn authed(self) -> Self;
}

impl AuthedRequest for Request {
	fn authed(self) -> Self {
		if let Some(cookie) = defaults_get::<String>(COOKIE_KEY).filter(|c| !c.is_empty()) {
			self.header("Cookie", &cookie)
		} else {
			self
		}
	}
}

fn authed_get(url: &str) -> Result<(String, Document)> {
	let base = base_url();
	let response = Request::get(url)?
		.authed()
		.header("User-Agent", "Aidoku")
		.header("Referer", &format!("{base}/"))
		.send()?;
	let raw = response.get_string()?;
	let doc = response.get_html()?;
	Ok((raw, doc))
}

fn xf_user_id_from_cookie_header(header: &str) -> Option<String> {
	for part in header.split(';') {
		let part = part.trim();
		let Some((name, value)) = part.split_once('=') else {
			continue;
		};
		if !name.eq_ignore_ascii_case("xf_user") || value.is_empty() {
			continue;
		}
		// Values are either `id` or `id,hash`.
		let id = value.split(',').next().unwrap_or(value).trim();
		if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
			return Some(id.into());
		}
	}
	None
}

/// Prefer Desu account menu markup: `.visitorText a.username`.
fn username_from_visitor_text(html: &str) -> Option<String> {
	let start = html.find("visitorText")?;
	let end = (start + 800).min(html.len());
	let slice = html.get(start..end)?;

	let mut search_from = 0;
	while let Some(rel) = slice[search_from..].find("<a ") {
		let a_start = search_from + rel;
		let a_tag = &slice[a_start..];
		let Some(gt) = a_tag.find('>') else {
			break;
		};
		let open = &a_tag[..=gt];
		if open.contains("username") {
			let rest = &a_tag[gt + 1..];
			if let Some(close) = rest.find('<') {
				let name = rest[..close].trim();
				if !name.is_empty() {
					return Some(name.into());
				}
			}
		}
		search_from = a_start + 3;
		if search_from >= slice.len() {
			break;
		}
	}

	// Fallback: members/{slug}.{id}/ inside visitorText.
	if let Some(href) = slice.find("members/") {
		let rest = &slice[href + "members/".len()..];
		let end = rest.find(['"', '\'', ' ', '/', '?']).unwrap_or(rest.len());
		let slug = &rest[..end];
		if let Some((name, id)) = slug.rsplit_once('.')
			&& !name.is_empty()
			&& id.chars().all(|c| c.is_ascii_digit())
		{
			return Some(name.into());
		}
	}
	None
}

fn username_from_dom(doc: &Document) -> Option<String> {
	const SELECTORS: &[&str] = &[
		".visitorText a.username",
		".visitorText .username",
		"a.username.NoOverlay",
		"a.username[href*='members/']",
		".username[href*='members/']",
		".p-navgroup-link--user .p-navgroup-linkText",
		".p-navgroup-link--user",
	];
	for sel in SELECTORS {
		if let Some(name) = doc
			.select_first(*sel)
			.and_then(|el| el.text())
			.map(|s| s.trim().to_string())
			.filter(|s| {
				!s.is_empty()
					&& !s.eq_ignore_ascii_case("войти")
					&& !s.contains("Вошли как")
					&& !s.contains("вошли как")
			}) {
			return Some(name);
		}
	}
	None
}

fn username_from_member_href(html: &str) -> Option<String> {
	// members/{slug}.{id}/
	let key = "members/";
	let mut from = 0;
	while let Some(rel) = html[from..].find(key) {
		let abs = from + rel + key.len();
		let rest = &html[abs..];
		let end = rest
			.find(['"', '\'', ' ', '?', '#'])
			.unwrap_or(rest.len().min(80));
		let slug = rest[..end].trim_end_matches('/');
		if let Some((name, id)) = slug.rsplit_once('.')
			&& !name.is_empty()
			&& id.chars().all(|c| c.is_ascii_digit())
		{
			return Some(name.into());
		}
		from = abs;
		if from >= html.len() {
			break;
		}
	}
	None
}

/// Fetches the current username for the settings status footer.
///
/// Desu exposes it in `.visitorText a.username` (account menu) and as `members/{name}.{id}/`.
pub fn refresh_username() -> Result<()> {
	let base = base_url();
	let cookie = defaults_get::<String>(COOKIE_KEY).unwrap_or_default();
	let user_id = xf_user_id_from_cookie_header(&cookie);

	// Account / profile pages usually contain visitorText for the signed-in user.
	let candidates = [
		format!("{base}/account/"),
		format!("{base}/"),
		user_id
			.as_ref()
			.map(|id| format!("{base}/members/{id}/"))
			.unwrap_or_default(),
	];

	for url in candidates {
		if url.is_empty() {
			continue;
		}
		let Ok((raw, doc)) = authed_get(&url) else {
			continue;
		};
		if let Some(name) = username_from_visitor_text(&raw)
			.or_else(|| username_from_dom(&doc))
			.or_else(|| username_from_member_href(&raw))
		{
			store_username(&name);
			return Ok(());
		}
	}
	Ok(())
}

/// Completes Aidoku web login by adopting XenForo cookies from the WebView.
///
/// Returns `true` when `xf_user` is present so ranobe/HTML routes can send `Cookie`.
pub fn handle_web_login(cookies: HashMap<String, String>) -> Result<bool> {
	let Some(header) = cookie_header_from_map(&cookies) else {
		return Ok(false);
	};

	store_cookie_header(&header);
	set_logged_in(true);
	set_just_logged_in();

	// Best-effort; notification handler also refreshes for DynamicSettings.
	let _ = refresh_username();

	Ok(true)
}

/// Gates ranobe scraping behind an existing Desu session.
pub fn require_login() -> Result<()> {
	if is_logged_in() {
		Ok(())
	} else {
		bail!("Войдите в аккаунт Desu в настройках источника")
	}
}
