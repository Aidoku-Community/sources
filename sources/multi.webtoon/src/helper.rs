use aidoku::{
	Result,
	alloc::{String, Vec, format},
	imports::{defaults::defaults_get, net::Request},
};

pub const BASE_URL_DESKTOP: &str = "https://www.webtoons.com";
pub const BASE_URL_MOBILE: &str = "https://m.webtoons.com";

/// Returns the currently selected language code, falling back to "en".
pub fn get_lang_code() -> String {
	if let Some(langs) = defaults_get::<Vec<String>>("languages")
		&& let Some(lang) = langs.into_iter().next()
	{
		return match lang.as_str() {
			"zh-Hant" => String::from("zh-hant"),
			_ => lang,
		};
	}
	String::from("en")
}

/// Returns the base URL without language path component.
pub fn get_base_url_no_lang(mobile: bool) -> &'static str {
	if mobile {
		BASE_URL_MOBILE
	} else {
		BASE_URL_DESKTOP
	}
}

/// Returns the localized base URL for the selected language.
pub fn get_base_url(mobile: bool) -> String {
	let base = get_base_url_no_lang(mobile);
	let lang = get_lang_code();
	format!("{base}/{lang}")
}

/// Returns the User-Agent header string.
pub fn get_user_agent(mobile: bool) -> &'static str {
	if mobile {
		"Mozilla/5.0 (iPhone; CPU iPhone OS 16_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.1 Mobile/15E148 Safari/604.1"
	} else {
		"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
	}
}

/// Request wrapper that sets cookies, referer, and user-agent.
pub fn request(url: &str, mobile: bool) -> Result<Request> {
	let locale = get_lang_code();
	let cookie_string = format!(
		"locale={locale}; ageGatePass=true; needGDPR=false; needCCPA=true; needCOPPA=false"
	);

	Ok(Request::get(url)?
		.header("Referer", get_base_url_no_lang(mobile))
		.header("Cookie", &cookie_string)
		.header("User-Agent", get_user_agent(mobile)))
}

/// Extracts a query parameter value from a URL, stopping at '&', '#', or '/'.
fn get_param<'a>(url: &'a str, param: &str) -> Option<&'a str> {
	let idx = url.find(param)?;
	let after = &url[idx + param.len()..];
	let end = after.find(['&', '#', '/']).unwrap_or(after.len());
	let val = &after[..end];
	if val.is_empty() { None } else { Some(val) }
}

/// Returns the ID of a manga from a URL.
pub fn get_manga_id(url: &str) -> Option<String> {
	let val = get_param(url, "title_no=").or_else(|| get_param(url, "titleNo="))?;

	if url.contains("canvas") || url.contains("challenge") {
		if !val.ends_with("-canvas") {
			Some(format!("{val}-canvas"))
		} else {
			Some(String::from(val))
		}
	} else {
		Some(String::from(val))
	}
}

/// Returns the ID of a chapter from a URL.
pub fn get_chapter_id(url: &str) -> Option<String> {
	let val = get_param(url, "episode_no=").or_else(|| get_param(url, "episodeNo="))?;

	if url.contains("canvas") || url.contains("challenge") {
		if !val.ends_with("-canvas") {
			Some(format!("{val}-canvas"))
		} else {
			Some(String::from(val))
		}
	} else {
		Some(String::from(val))
	}
}

/// Returns full URL of a manga from a manga ID.
pub fn get_manga_url(manga_id: &str) -> String {
	if let Some(canvas_id) = manga_id.strip_suffix("-canvas") {
		format!("{BASE_URL_DESKTOP}/challenge/episodeList?titleNo={canvas_id}")
	} else {
		format!("{BASE_URL_DESKTOP}/episodeList?titleNo={manga_id}")
	}
}

/// Returns full URL of a chapter from a chapter ID and manga ID.
pub fn get_chapter_url(chapter_id: &str, manga_id: &str) -> String {
	let clean_manga_id = manga_id.strip_suffix("-canvas").unwrap_or(manga_id);
	let clean_chapter_id = chapter_id.strip_suffix("-canvas").unwrap_or(chapter_id);

	if manga_id.ends_with("-canvas") || chapter_id.ends_with("-canvas") {
		format!(
			"{BASE_URL_DESKTOP}/challenge/viewer?titleNo={clean_manga_id}&episodeNo={clean_chapter_id}"
		)
	} else {
		format!("{BASE_URL_DESKTOP}/viewer?titleNo={manga_id}&episodeNo={chapter_id}")
	}
}

/// Returns whether canvas series are enabled in settings.
pub fn get_canvas_series() -> bool {
	defaults_get::<bool>("canvasSeries").unwrap_or(true)
}

/// Returns whether optional pages should be displayed.
pub fn get_optional_pages() -> bool {
	defaults_get::<bool>("optionalPages").unwrap_or(true)
}
