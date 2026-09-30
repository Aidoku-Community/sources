use aidoku::{Chapter, alloc::*};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
pub struct VChapter {
	pub chapter_name: Option<String>,
	pub chapter_num: Option<f32>,
	pub chapter_slug: String,
	pub updated_at: Option<DateTime<Utc>>,
}

impl From<VChapter> for Chapter {
	fn from(value: VChapter) -> Self {
		Self {
			key: value
				.chapter_num
				.map(|v| format!("chuong-{v}"))
				.unwrap_or(value.chapter_slug),
			title: value.chapter_name,
			chapter_number: value.chapter_num,
			date_uploaded: value.updated_at.map(|v| v.timestamp()),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct Chapters {
	pub chapters: Vec<VChapter>,
	pub current_page: usize,
	pub last_page: usize,
}

#[derive(Deserialize)]
pub struct ChaptersData {
	pub data: Chapters,
}
