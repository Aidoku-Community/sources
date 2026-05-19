use crate::net::BASE_URL;
use aidoku::{
	Chapter, FilterValue,
	alloc::{String, Vec, format, string::ToString},
};

pub fn get_absolute_url(url: &str) -> String {
	if url.starts_with("//") {
		format!("https:{}", url)
	} else if url.starts_with('/') {
		format!("{}{}", BASE_URL, url)
	} else {
		url.to_string()
	}
}

pub fn extract_key(url: &str) -> Option<String> {
	let clean = url.trim_end_matches('/');
	let segment = clean.split('/').next_back()?;
	if segment.chars().all(|c| c.is_ascii_digit()) {
		Some(segment.to_string())
	} else {
		None
	}
}

pub fn extract_chapter_key(url: &str) -> Option<String> {
	let clean = url.trim_end_matches('/');
	let segment = clean.split('/').next_back()?;
	if segment.ends_with(".html") {
		Some(segment.to_string())
	} else {
		None
	}
}

pub fn extract_chapter_number(title: &str) -> Option<f32> {
	let mut num_str = String::new();
	let mut found_digit = false;
	for c in title.chars() {
		if c.is_ascii_digit() || (c == '.' && found_digit && !num_str.contains('.')) {
			num_str.push(c);
			found_digit = true;
		} else if found_digit {
			break;
		}
	}
	num_str.parse::<f32>().ok()
}

pub fn get_search_url(query: Option<String>, page: i32, filters: Vec<FilterValue>) -> String {
	if let Some(q) = query
		&& !q.is_empty()
	{
		let encoded = aidoku::helpers::uri::encode_uri(q);
		return if page <= 1 {
			format!("{}/search/{}", BASE_URL, encoded)
		} else {
			format!("{}/search/{}/{}", BASE_URL, encoded, page)
		};
	}

	let mut leaderboard_id = String::new();
	let mut status_id = String::new();
	let mut audience_id = String::new();

	for filter in filters {
		if let FilterValue::Select { id, value } = filter {
			if id == "leaderboard" {
				leaderboard_id = value;
			} else if id == "status" {
				status_id = value;
			} else if id == "audience" {
				audience_id = value;
			}
		}
	}

	if !leaderboard_id.is_empty() {
		return format!("{}/custom/{}?page={}", BASE_URL, leaderboard_id, page);
	}

	let mut url = format!("{}/category", BASE_URL);
	if !status_id.is_empty() {
		url = format!("{}/finish/{}", url, status_id);
	}
	if !audience_id.is_empty() {
		url = format!("{}/list/{}", url, audience_id);
	}

	format!("{}?page={}", url, page)
}

pub fn get_chapter_url(chapter: &Chapter) -> String {
	if let Some(ref u) = chapter.url
		&& !u.is_empty()
	{
		return u.clone();
	}
	if chapter.key.starts_with("http") {
		chapter.key.clone()
	} else if chapter.key.contains("/show/") {
		get_absolute_url(&chapter.key)
	} else {
		get_absolute_url(&format!("/show/{}", chapter.key))
	}
}
