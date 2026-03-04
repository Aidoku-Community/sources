use aidoku::{
	alloc::{Vec, string::String},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	prelude::*,
};

const DOMAIN_KEY: &str = "domain";
const IPB_MEMBER_ID_KEY: &str = "ipb_member_id";
const IPB_PASS_HASH_KEY: &str = "ipb_pass_hash";
const IGNEOUS_KEY: &str = "igneous";
const TITLE_PREFERENCE_KEY: &str = "titlePreference";
const LANGUAGES_KEY: &str = "language";
const BLOCKLIST_KEY: &str = "blocklist";

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TitlePreference {
	#[default]
	English,
	Japanese,
}

pub fn get_domain() -> String {
	defaults_get::<String>(DOMAIN_KEY).unwrap_or_else(|| "e-hentai.org".into())
}

pub fn get_base_url() -> String {
	let domain = get_domain();
	format!("https://{}", domain)
}

pub fn get_ipb_member_id() -> String {
	defaults_get::<String>(IPB_MEMBER_ID_KEY).unwrap_or_default()
}

pub fn get_ipb_pass_hash() -> String {
	defaults_get::<String>(IPB_PASS_HASH_KEY).unwrap_or_default()
}

pub fn get_igneous() -> String {
	defaults_get::<String>(IGNEOUS_KEY).unwrap_or_default()
}

pub fn get_title_preference() -> TitlePreference {
	defaults_get::<String>(TITLE_PREFERENCE_KEY)
		.map(|v| match v.as_str() {
			"japanese" => TitlePreference::Japanese,
			_ => TitlePreference::English,
		})
		.unwrap_or_default()
}

/// Returns the tag blocklist as lowercase strings.
/// Items can be plain names (e.g. `"guro"`) or namespace-prefixed (e.g. `"female:guro"`).
pub fn get_blocklist() -> Vec<String> {
	defaults_get::<Vec<String>>(BLOCKLIST_KEY)
		.unwrap_or_default()
		.into_iter()
		.map(|s| s.trim().to_lowercase())
		.filter(|s| !s.is_empty())
		.collect()
}

/// Returns the E-Hentai language tag name for the user-selected language, if any.
pub fn get_language_filter() -> Option<String> {
	defaults_get::<String>(LANGUAGES_KEY).and_then(|lang| match lang.as_str() {
		"ja" => Some("japanese".into()),
		"en" => Some("english".into()),
		"zh" => Some("chinese".into()),
		"nl" => Some("dutch".into()),
		"fr" => Some("french".into()),
		"de" => Some("german".into()),
		"hu" => Some("hungarian".into()),
		"it" => Some("italian".into()),
		"ko" => Some("korean".into()),
		"pl" => Some("polish".into()),
		"pt-BR" => Some("portuguese".into()),
		"ru" => Some("russian".into()),
		"es" => Some("spanish".into()),
		"th" => Some("thai".into()),
		"vi" => Some("vietnamese".into()),
		_ => None,
	})
}

pub fn build_cookie_header() -> String {
	let member_id = get_ipb_member_id();
	let pass_hash = get_ipb_pass_hash();
	let igneous = get_igneous();

	let mut parts = Vec::new();

	parts.push("nw=1".into());

	if !member_id.is_empty() {
		parts.push(format!("ipb_member_id={}", member_id));
	}
	if !pass_hash.is_empty() {
		parts.push(format!("ipb_pass_hash={}", pass_hash));
	}
	if !igneous.is_empty() {
		parts.push(format!("igneous={}", igneous));
	}

	parts.join("; ")
}

fn cursor_key(listing_id: &str) -> String {
	format!("cursor_{}", listing_id)
}

pub fn set_page_cursor(listing_id: &str, gid: &str) {
	defaults_set(&cursor_key(listing_id), DefaultValue::String(gid.into()));
}

pub fn get_page_cursor(listing_id: &str) -> Option<String> {
	defaults_get::<String>(&cursor_key(listing_id)).filter(|s| !s.is_empty())
}

pub fn clear_page_cursor(listing_id: &str) {
	defaults_set(&cursor_key(listing_id), DefaultValue::Null);
}

pub fn refresh_igneous_from_set_cookie(set_cookie: &str) {
	for part in set_cookie.split([',', '\n']) {
		let part = part.trim();
		if part.starts_with("igneous=") {
			if let Some(value) = part
				.trim_start_matches("igneous=")
				.split(';')
				.next()
				.map(str::trim)
				.filter(|v| !v.is_empty())
			{
				defaults_set(IGNEOUS_KEY, DefaultValue::String(value.into()));
			}
			return;
		}
	}
}
