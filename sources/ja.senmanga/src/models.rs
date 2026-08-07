use aidoku::alloc::{String, Vec};
use serde::Deserialize;

/// Response of `/api/directory`, used for both browsing and searching.
///
/// `currentPage` and `totalPages` are null whenever the result fits on a
/// single page, and `totalPages` is a pager window rather than the real total,
/// so it may grow as later pages are requested.
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

/// Response of `/api/manga/{slug}`.
#[derive(Deserialize)]
pub struct MangaDetails {
	pub title: String,
	pub cover: Option<String>,
	/// Comma-separated genre names.
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
	/// Chapter identifier used by `/api/read/{slug}/{url}`, e.g. "8.338323".
	pub url: String,
	pub full_url: Option<String>,
	/// ISO 8601 timestamp, e.g. "2026-08-06T12:04:20Z".
	pub datetime: Option<String>,
}

/// Response of `/api/read/{slug}/{chapter}`.
#[derive(Deserialize)]
pub struct ReadResponse {
	/// Absolute image urls.
	#[serde(default)]
	pub pages: Vec<String>,
}
