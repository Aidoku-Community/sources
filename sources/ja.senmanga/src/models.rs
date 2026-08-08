use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DirectoryResponse {
	#[serde(rename = "currentPage")]
	pub current_page: Option<i32>,
	#[serde(rename = "totalPages")]
	pub total_pages: Option<i32>,
	#[serde(default)]
	pub series: Vec<SeriesEntry>,
}

#[derive(Deserialize)]
pub struct SeriesEntry {
	pub title: String,
	pub slug: String,
	pub cover: Option<String>,
	pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct MangaDetails {
	pub title: String,
	pub cover: Option<String>,
	pub genre: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub status: Option<String>,
	pub description: Option<String>,
	#[serde(rename = "chapterList", default)]
	pub chapter_list: Vec<ChapterEntry>,
}

#[derive(Deserialize)]
pub struct ChapterEntry {
	pub title: Option<String>,
	pub number: Option<String>,
	pub url: String,
	pub full_url: Option<String>,
	pub datetime: Option<String>,
}

#[derive(Deserialize)]
pub struct ReadResponse {
	#[serde(default)]
	pub pages: Vec<String>,
}
