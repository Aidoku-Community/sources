use aidoku::{
	alloc::{String, format},
	imports::{error::Result, net::Request, std::parse_date},
};

pub const BASE_URL: &str = "https://m.comic.naver.com";
pub const API_URL: &str = "https://comic.naver.com/api";

pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

fn is_naver_host(url: &str) -> bool {
	let host = url
		.strip_prefix("https://")
		.or_else(|| url.strip_prefix("http://"))
		.unwrap_or(url)
		.split(['/', '?', '#', ':'])
		.next()
		.unwrap_or("");
	host == "naver.com" || host.ends_with(".naver.com")
}

/// Request wrapper with User-Agent, Referer, Accept headers.
pub fn request(url: &str) -> Result<Request> {
	let mut req = Request::get(url)?
		.header("Referer", "https://comic.naver.com/")
		.header("User-Agent", USER_AGENT);

	if is_naver_host(url)
		&& let Some(cookie) = crate::auth::get_cookie_header()
	{
		req = req.header("Cookie", &cookie);
	}

	if url.contains("/api/") {
		req = req.header("Accept", "application/json");
	} else {
		req = req.header(
			"Accept",
			"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
		);
	}

	Ok(req)
}

fn get_param<'a>(url: &'a str, param: &str) -> Option<&'a str> {
	let mut query_start = false;
	let mut val = "";
	for part in url.split(['?', '&']) {
		if !query_start {
			query_start = true;
			continue;
		}
		let clean = part.trim_start_matches("amp;");
		if let Some(v) = clean.strip_prefix(param) {
			val = v.split('#').next().unwrap_or(v);
			break;
		}
	}
	if val.is_empty() { None } else { Some(val) }
}

/// Extracts titleId from a given webtoon URL
pub fn get_title_id(url: &str) -> Option<String> {
	let id_str = get_param(url, "titleId=")?;
	if url.contains("bestChallenge") {
		Some(format!("{id_str}-best"))
	} else {
		Some(String::from(id_str))
	}
}

/// Extracts episode sequence number 'no' from a viewer or detail URL
pub fn get_chapter_id(url: &str) -> Option<String> {
	let val = get_param(url, "no=")?;
	Some(String::from(val))
}

/// Returns full list URL for a manga
pub fn get_manga_url(manga_id: &str) -> String {
	if let Some(clean_id) = manga_id.strip_suffix("-best") {
		format!("{BASE_URL}/bestChallenge/list?titleId={clean_id}")
	} else {
		format!("{BASE_URL}/webtoon/list?titleId={manga_id}")
	}
}

/// Returns full viewer URL for a chapter
pub fn get_chapter_url(chapter_id: &str, manga_id: &str) -> String {
	if let Some(clean_id) = manga_id.strip_suffix("-best") {
		format!("{BASE_URL}/bestChallenge/detail?titleId={clean_id}&no={chapter_id}")
	} else {
		format!("{BASE_URL}/webtoon/detail?titleId={manga_id}&no={chapter_id}")
	}
}

fn extract_number_before(title: &str, marker: char) -> Option<f32> {
	let idx = title.find(marker)?;
	let before = &title[..idx];
	let end = before
		.char_indices()
		.rev()
		.find(|(_, c)| c.is_ascii_digit())
		.map(|(i, c)| i + c.len_utf8())?;
	let start = before[..end]
		.char_indices()
		.rev()
		.find(|(_, c)| !c.is_ascii_digit() && *c != '.')
		.map(|(i, c)| i + c.len_utf8())
		.unwrap_or(0);
	before[start..end].parse::<f32>().ok()
}

/// Extracts numeric chapter value from title
pub fn extract_chapter_number(title: &str, fallback_no: f32) -> f32 {
	extract_number_before(title, '화').unwrap_or(fallback_no)
}

/// Extracts season/volume value from title
pub fn extract_volume_number(title: &str) -> Option<f32> {
	extract_number_before(title, '부')
}

/// Parses Korean date string ("YY.MM.DD" or "YYYY.MM.DD") into unix timestamp (seconds)
pub fn parse_korean_date(date_str: &str) -> Option<i64> {
	let trimmed = date_str.trim().trim_end_matches('.');
	if trimmed.len() <= 8 {
		parse_date(trimmed, "yy.MM.dd")
	} else {
		parse_date(trimmed, "yyyy.MM.dd")
	}
}
