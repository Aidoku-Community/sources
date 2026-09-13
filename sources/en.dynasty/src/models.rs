//! JSON models for dynasty-scans.com's unofficial endpoints.
//!
//! These endpoints are not a documented API; they are the same payloads the
//! website's own frontend consumes. Unknown fields are ignored.

use aidoku::alloc::{String, Vec};
use serde::Deserialize;

/// A tag object, used across all endpoints.
#[derive(Deserialize)]
pub struct Tag {
	#[serde(default)]
	pub name: String,
	/// e.g. "Series", "Anthology", "Doujin", "Author", "Artist", "Scanlator",
	/// "General", "Status", "Pairing"
	#[serde(default, rename = "type")]
	pub tag_type: String,
	#[serde(default)]
	pub permalink: Option<String>,
}

/// GET /chapters/added.json
#[derive(Deserialize)]
pub struct AddedResponse {
	#[serde(default)]
	pub chapters: Vec<AddedChapter>,
	#[serde(default)]
	pub total_pages: i32,
}

#[derive(Deserialize)]
pub struct AddedChapter {
	#[serde(default)]
	pub permalink: String,
	#[serde(default)]
	pub tags: Vec<Tag>,
}

/// GET /{id}.json — series, doujin, or anthology details.
#[derive(Deserialize)]
pub struct SiteItemJson {
	#[serde(default)]
	pub name: String,
	#[serde(default)]
	pub cover: Option<String>,
	#[serde(default)]
	pub description: Option<String>,
	/// Alternative titles; observed as either a string or a string array
	/// depending on the item.
	#[serde(default)]
	pub aliases: Option<serde_json::Value>,
	#[serde(default)]
	pub tags: Vec<Tag>,
	/// Chapter/update list, newest first. Contains `header` entries to mark
	/// volume boundaries between chapter entries.
	#[serde(default)]
	pub taggings: Vec<Tagging>,
}

#[derive(Deserialize)]
pub struct Tagging {
	/// Present on volume divider entries (e.g. "Volume 3").
	#[serde(default)]
	pub header: Option<String>,
	#[serde(default)]
	pub title: Option<String>,
	#[serde(default)]
	pub permalink: Option<String>,
	/// "YYYY-MM-DD"
	#[serde(default)]
	pub released_on: Option<String>,
	#[serde(default)]
	pub tags: Vec<Tag>,
}

/// GET /chapters/{permalink}.json
#[derive(Deserialize)]
pub struct ChapterJson {
	#[serde(default)]
	pub tags: Vec<Tag>,
	#[serde(default)]
	pub pages: Vec<PageJson>,
}

#[derive(Deserialize)]
pub struct PageJson {
	/// Relative image path, e.g. "/system/releases/000/048/309/e_0001.webp".
	#[serde(default)]
	pub url: String,
}
