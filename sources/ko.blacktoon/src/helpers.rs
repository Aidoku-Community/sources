use crate::models::{Hosts, SeriesItem};
use aidoku::{
	alloc::{String, Vec, format},
	bail,
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		error::Result,
		net::Request,
		std::parse_date,
	},
};

pub const DEFAULT_BASE_URL: &str = "https://blacktoon428.com";
pub const ENTRY_URL: &str = "https://blacktoon.me";
pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148";

/// Returns the user's custom base URL from settings if non-empty,
/// otherwise dynamically resolves the latest domain via ENTRY_URL redirect,
/// falling back to DEFAULT_BASE_URL when unreachable (e.g. HTTP 451).
pub fn get_base_url() -> String {
	if let Some(url) = defaults_get::<String>("baseUrl") {
		let clean = url.trim().trim_end_matches('/');
		if !clean.is_empty() {
			return clean.into();
		}
	}

	resolve_final_redirect(ENTRY_URL).unwrap_or_else(|| String::from(DEFAULT_BASE_URL))
}

pub fn set_base_url(url: &str) {
	let clean = url.trim().trim_end_matches('/');
	defaults_set("baseUrl", DefaultValue::String(String::from(clean)));
}

pub fn request_with_referer(url: &str, referer: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", referer))
}

pub fn request(url: &str) -> Result<Request> {
	let base_url = get_base_url();
	request_with_referer(url, &base_url)
}

pub fn fetch_hosts(base_url: &str) -> Hosts {
	let home = request_with_referer(base_url, base_url)
		.and_then(|r| r.string())
		.unwrap_or_default();
	let config_url = format!("{}/data/config.js", base_url.trim_end_matches('/'));
	let config = request_with_referer(&config_url, base_url)
		.and_then(|r| r.string())
		.unwrap_or_default();

	let toon_list = extract_js_var(&home, "inc_url1")
		.unwrap_or("https://bjst.speedwebgo.com")
		.trim_end_matches('/')
		.into();

	let webtoon = extract_js_var(&home, "inc_url2")
		.unwrap_or("https://bjst.speedwebgo.com")
		.trim_end_matches('/')
		.into();

	let image = extract_js_var(&config, "img_domain")
		.unwrap_or("https://diksds.speedwebgo.com")
		.trim_end_matches('/')
		.into();

	Hosts {
		toon_list,
		webtoon,
		image,
	}
}

pub fn fetch_series_list(url: &str, is_completed: bool) -> Result<Vec<SeriesItem>> {
	let js = request(url)?.string()?;
	let Some(json_str) = extract_js_array(&js) else {
		bail!("웹툰 목록 데이터를 불러올 수 없습니다.");
	};
	let mut items: Vec<SeriesItem> = serde_json::from_str(json_str)?;
	for item in &mut items {
		item.is_completed = is_completed;
	}
	Ok(items)
}

pub fn resolve_final_redirect(start_url: &str) -> Option<String> {
	let resp = Request::get(start_url)
		.ok()?
		.header("User-Agent", USER_AGENT)
		.send()
		.ok()?;
	let final_url = resp.get_url()?;
	let (scheme, rest) = final_url.split_once("://")?;
	let host = rest
		.split(['/', '?', '#'])
		.next()
		.filter(|h| !h.is_empty())?;
	Some(format!("{scheme}://{host}"))
}

pub fn extract_js_var<'a>(src: &'a str, name: &str) -> Option<&'a str> {
	let after = src.split_once(name)?.1.trim_start();
	let after_eq = after.strip_prefix('=')?.trim_start();
	let quote = after_eq.chars().next().filter(|&c| c == '"' || c == '\'')?;
	after_eq
		.strip_prefix(quote)?
		.split_once(quote)
		.map(|(v, _)| v)
}

pub fn extract_js_array(src: &str) -> Option<&str> {
	let start = src.find('[')?;
	let end = src.rfind(']')?;
	(start < end).then(|| &src[start..=end])
}

pub fn extract_chapter_number(title: &str) -> Option<f32> {
	let (before, _) = title.split_once('화')?;
	before
		.split(|c: char| !c.is_ascii_digit() && c != '.')
		.rfind(|s| !s.is_empty())?
		.parse()
		.ok()
}

pub fn parse_date_to_epoch(date_str: &str) -> Option<i64> {
	let trimmed = date_str.split_whitespace().next().unwrap_or(date_str);
	parse_date(trimmed, "yyyy-MM-dd")
}
