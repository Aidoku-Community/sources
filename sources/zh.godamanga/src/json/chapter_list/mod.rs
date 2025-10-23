use crate::{API_URL, BASE_URL};
use aidoku::{
	alloc::{string::ToString as _, vec, Vec},
	error,
	imports::net::Request,
	prelude::format,
	Chapter, Result,
};
use chrono::DateTime;
use regex::Regex;

fn extract_chapter_number(title: &str) -> Option<f32> {
	// This handles cases like "第183话 180" where 180 is the actual chapter
	let re1 =
		Regex::new(r"(?:第\s*\d+(?:\.\d+)?\s*(?:话|話|章|回|卷|册|冊)\s*)(\d+(?:\.\d+)?)").ok()?;
	if let Some(captures) = re1.captures(title) {
		if let Some(num_match) = captures.get(1) {
			if let Ok(num) = num_match.as_str().parse::<f32>() {
				return Some(num);
			}
		}
	}

	// Second try: match "第X话" pattern where X is the chapter number
	let re2 = Regex::new(r"(?:第\s*)(\d+(?:\.\d+)?)\s*(?:话|話|章|回|卷|册|冊)").ok()?;
	if let Some(captures) = re2.captures(title) {
		if let Some(num_match) = captures.get(1) {
			if let Ok(num) = num_match.as_str().parse::<f32>() {
				return Some(num);
			}
		}
	}

	// Third try: match pure number at the beginning
	let re3 = Regex::new(r"^(\d+(?:\.\d+)?)").ok()?;
	if let Some(captures) = re3.captures(title) {
		if let Some(num_match) = captures.get(1) {
			if let Ok(num) = num_match.as_str().parse::<f32>() {
				return Some(num);
			}
		}
	}

	None
}

pub struct ChapterList;

impl ChapterList {
	pub fn get_chapters(manga_id: &str) -> Result<Vec<Chapter>> {
		let ids = manga_id.split("/").collect::<Vec<&str>>();
		let url = format!("{}/api/manga/get?mid={}&mode=all", API_URL, ids[1]);
		let json: serde_json::Value = Request::get(url.clone())?
			.header("Origin", BASE_URL)
			.header("Referer", BASE_URL)
			.send()?
			.get_json()?;
		let data = json
			.as_object()
			.ok_or_else(|| error!("Expected JSON object"))?;
		let data = data
			.get("data")
			.and_then(|v| v.as_object())
			.ok_or_else(|| error!("Expected data object"))?;
		let list = data
			.get("chapters")
			.and_then(|v| v.as_array())
			.ok_or_else(|| error!("Expected chapters array"))?;
		let mut chapters: Vec<Chapter> = Vec::new();

		for (index, item) in list.iter().enumerate() {
			let item = match item.as_object() {
				Some(item) => item,
				None => continue,
			};
			let attributes = match item.get("attributes").and_then(|v| v.as_object()) {
				Some(attrs) => attrs,
				None => continue,
			};

			let id = match item.get("id") {
				Some(id_value) => {
					if let Some(id_str) = id_value.as_str() {
						id_str.to_string()
					} else if let Some(id_num) = id_value.as_i64() {
						id_num.to_string()
					} else {
						continue; // Skip this item if ID is neither string nor number
					}
				}
				None => continue, // Skip if no ID field
			};

			let title = attributes
				.get("title")
				.and_then(|v| v.as_str())
				.unwrap_or("Unknown")
				.to_string();
			let slug = attributes
				.get("slug")
				.and_then(|v| v.as_str())
				.unwrap_or(&id)
				.to_string();
			let url = format!("{}/manga/{}/{}", BASE_URL, ids[0], slug);
			let chapter = (index + 1) as f32;
			let chapter_or_volume = extract_chapter_number(&title).unwrap_or(chapter);

			let (ch, vo) = if title.trim().ends_with('卷') {
				(-1.0, chapter_or_volume)
			} else {
				(chapter_or_volume, -1.0)
			};

			let scanlator = if vo > -1.0 {
				"单行本".to_string()
			} else {
				"默认".to_string()
			};

			// Parse updatedAt timestamp
			let date_uploaded = attributes
				.get("updatedAt")
				.and_then(|v| v.as_str())
				.and_then(|date_str| DateTime::parse_from_rfc3339(date_str).ok())
				.map(|dt| dt.timestamp());

			chapters.push(aidoku::Chapter {
				key: id,
				title: Some(title),
				volume_number: (vo >= 0.0).then_some(vo),
				chapter_number: (ch >= 0.0).then_some(ch),
				date_uploaded,
				url: Some(url),
				scanlators: Some(vec![scanlator]),
				..Default::default()
			});
		}
		chapters.reverse();

		Ok(chapters)
	}
}
