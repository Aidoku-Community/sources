use crate::BASE_URL;
use aidoku::{
	alloc::{string::ToString as _, Vec, vec},
	imports::net::{HttpMethod, Request},
	prelude::format,
	AidokuError, Chapter, Result,
};
use regex::Regex;

fn extract_chapter_number(title: &str) -> Option<f32> {
	let re1 = Regex::new(r"(?:第\s*)(\d+(?:\.\d+)?)\s*(?:话|話|章|回|卷|册|冊)").ok()?;
	if let Some(captures) = re1.captures(title) {
		if let Some(num_match) = captures.get(1) {
			if let Ok(num) = num_match.as_str().parse::<f32>() {
				return Some(num);
			}
		}
	}

	// Try to match pure number at the beginning
	let re2 = Regex::new(r"^(\d+(?:\.\d+)?)").ok()?;
	if let Some(captures) = re2.captures(title) {
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
		let mut all_chapters: Vec<Chapter> = Vec::new();
		let mut page = 1;

		loop {
			let url = format!(
				"{}/v2.0/apis/manga/chapterByPage?code={}&page={}&lang=cn&order=asc",
				BASE_URL, manga_id, page
			);
			let json: serde_json::Value = Request::new(url.clone(), HttpMethod::Get)?
				.header("Origin", BASE_URL)
				.header("Referer", BASE_URL)
				.send()?
				.get_json()?;
			let data = json
				.as_object()
				.ok_or_else(|| AidokuError::message("Expected JSON object"))?;
			let data = data
				.get("data")
				.and_then(|v| v.as_object())
				.ok_or_else(|| AidokuError::message("Expected data object"))?;
			let is_end = data.get("isEnd").and_then(|v| v.as_i64()).unwrap_or(0);
			let items = data
				.get("items")
				.and_then(|v| v.as_array())
				.ok_or_else(|| AidokuError::message("Expected items array"))?;

			for item in items {
				let item = match item.as_object() {
					Some(item) => item,
					None => continue,
				};
				let id = item
					.get("codes")
					.and_then(|v| v.as_str())
					.unwrap_or_default()
					.to_string();
				let title = item
					.get("chapterName")
					.and_then(|v| v.as_str())
					.unwrap_or_default()
					.to_string();
				let chapter_or_volume =
					extract_chapter_number(&title).unwrap_or((all_chapters.len() + 1) as f32);
				let url = format!("{}/mangaread/{}", BASE_URL, id.clone());

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

				all_chapters.push(aidoku::Chapter {
					key: id,
					title: Some(title),
					volume_number: if vo >= 0.0 { Some(vo) } else { None },
					chapter_number: if ch >= 0.0 { Some(ch) } else { None },
					url: Some(url),
					scanlators: Some(vec![scanlator]),
					..Default::default()
				});
			}

			if is_end == 1 {
				break;
			}
			page += 1;
		}

		all_chapters.reverse();

		Ok(all_chapters)
	}
}
