use aidoku::{
	alloc::{String, format},
	imports::{defaults::defaults_get, error::Result, net::Request, std::parse_date},
};

pub const BASE_URL: &str = "https://m.comic.naver.com";

pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

/// Request wrapper with User-Agent, Referer, and automatic Cookie injection
pub fn request(url: &str) -> Result<Request> {
	let mut req = Request::get(url)?
		.header("Referer", "https://comic.naver.com/")
		.header("User-Agent", USER_AGENT);
	if let Some(cookie_str) = crate::auth::get_cookie_header() {
		req = req.header("Cookie", &cookie_str);
	}
	Ok(req)
}

/// Extracts titleId from a given webtoon URL
pub fn get_title_id(url: &str) -> String {
	if let Some(pos) = url.find("titleId=") {
		let after = &url[pos + 8..];
		let id_str = after.split('&').next().unwrap_or(after);
		if url.contains("bestChallenge") {
			format!("{id_str}-best")
		} else {
			String::from(id_str)
		}
	} else {
		String::new()
	}
}

/// Extracts episode sequence number 'no' from a viewer or detail URL
pub fn get_chapter_id(url: &str) -> String {
	if !url.contains("detail?") {
		return String::new();
	}
	let query = url.split('?').nth(1).unwrap_or("");
	for param in query.split('&') {
		let clean = param.trim_start_matches("amp;");
		if let Some(val) = clean.strip_prefix("no=") {
			let clean_val = val.split('#').next().unwrap_or(val);
			return String::from(clean_val);
		}
	}
	String::new()
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

/// Extracts numeric chapter value from title
pub fn extract_chapter_number(title: &str, fallback_no: f32) -> f32 {
	if let Some(hwa_idx) = title.find('화') {
		let before = &title[..hwa_idx];
		let mut num_str = String::new();
		for c in before.chars().rev() {
			if c.is_ascii_digit() || c == '.' {
				num_str.push(c);
			} else if !num_str.is_empty() {
				break;
			}
		}
		if !num_str.is_empty() {
			let reversed: String = num_str.chars().rev().collect();
			if let Ok(val) = reversed.parse::<f32>() {
				return val;
			}
		}
	}
	fallback_no
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

/// Check setting for best challenge
pub fn show_best_challenge() -> bool {
	defaults_get::<bool>("showBestChallenge").unwrap_or(true)
}
