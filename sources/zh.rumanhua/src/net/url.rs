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
	let path_idx = clean.rfind('/')?;
	let segment = &clean[path_idx + 1..];
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
	if let Some(q) = query {
		let encoded = aidoku::helpers::uri::encode_uri(q);
		return if page <= 1 {
			format!("{}/search/{}", BASE_URL, encoded)
		} else {
			format!("{}/search/{}/{}", BASE_URL, encoded, page)
		};
	}

	let mut leaderboard_id: Option<String> = None;
	let mut status_id: Option<String> = None;
	let mut audience_id: Option<String> = None;

	for filter in filters {
		if let FilterValue::Select { id, value } = filter {
			if id == "leaderboard" {
				leaderboard_id = Some(value);
				break;
			} else if id == "status" {
				status_id = Some(value);
			} else if id == "audience" {
				audience_id = Some(value);
			}
		}
	}

	if let Some(id) = leaderboard_id {
		return format!("{}/custom/{}?page={}", BASE_URL, id, page);
	}

	let mut url = format!("{}/category", BASE_URL);
	if let Some(id) = status_id {
		url = format!("{}/finish/{}", url, id);
	}
	if let Some(id) = audience_id {
		url = format!("{}/list/{}", url, id);
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
