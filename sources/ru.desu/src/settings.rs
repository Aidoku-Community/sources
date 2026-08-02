use aidoku::imports::defaults::defaults_get;
use alloc::format;
use alloc::string::String;

const ENGLISH_TITLE_KEY: &str = "englishTitles";
const DOMAIN_KEY: &str = "domain";
const DEFAULT_DOMAIN: &str = "desu.uno";

/// Prefer English titles when the catalog entry has both RU and EN names.
pub fn eng_title() -> bool {
	defaults_get::<bool>(ENGLISH_TITLE_KEY).unwrap_or(false)
}

/// Host only, e.g. `desu.uno` — strips scheme/path if the user pasted a full URL.
pub fn domain() -> String {
	normalize_domain(&defaults_get::<String>(DOMAIN_KEY).unwrap_or_else(|| DEFAULT_DOMAIN.into()))
}

fn normalize_domain(raw: &str) -> String {
	let trimmed = raw.trim();
	let without_scheme = trimmed
		.strip_prefix("https://")
		.or_else(|| trimmed.strip_prefix("http://"))
		.unwrap_or(trimmed);
	let host = without_scheme
		.split('/')
		.next()
		.unwrap_or(without_scheme)
		.trim()
		.trim_start_matches("www.");
	if host.is_empty() {
		DEFAULT_DOMAIN.into()
	} else {
		host.into()
	}
}

pub fn base_url() -> String {
	format!("https://{}", domain())
}

/// CDN host derived from the configured domain (`static.{domain}`).
pub fn static_base_url() -> String {
	format!("https://static.{}", domain())
}

pub fn ranobe_cover_preview(id: &str) -> String {
	format!("{}/data/ranobe/covers/preview/{id}.jpg", static_base_url())
}

/// Path after scheme+host when the host matches the configured domain.
pub fn path_on_site(url: &str) -> Option<String> {
	let host = domain();
	let rest = url
		.strip_prefix("https://")
		.or_else(|| url.strip_prefix("http://"))?;
	let (url_host, path) = match rest.split_once('/') {
		Some((h, p)) => (h, p),
		None => (rest, ""),
	};
	let url_host = url_host.trim_start_matches("www.");
	if !url_host.eq_ignore_ascii_case(&host) {
		return None;
	}
	Some(path.into())
}

/// Rewrite absolute static CDN URLs onto the configured `static.{domain}`.
pub fn rewrite_media_url(url: &str) -> String {
	let Some(rest) = url
		.strip_prefix("https://")
		.or_else(|| url.strip_prefix("http://"))
	else {
		return url.into();
	};
	let Some((host, path)) = rest.split_once('/') else {
		return url.into();
	};
	if host.starts_with("static.") {
		format!("{}/{}", static_base_url(), path)
	} else if host.eq_ignore_ascii_case(&domain())
		|| host.eq_ignore_ascii_case(&format!("www.{}", domain()))
	{
		format!("{}/{}", base_url(), path)
	} else {
		url.into()
	}
}
