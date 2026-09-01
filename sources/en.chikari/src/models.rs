use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ListResponse {
	#[serde(default)]
	pub items: Vec<NovelListItem>,
	#[serde(default)]
	pub total: u32,
	#[serde(default)]
	pub limit: u32,
	#[serde(default)]
	pub offset: u32,
}

#[derive(Deserialize)]
pub struct NovelListItem {
	pub slug: String,
	pub title: String,
	#[serde(default)]
	pub cover_url: Option<String>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub is_nsfw: bool,
}

#[derive(Deserialize)]
pub struct NovelDetail {
	pub slug: String,
	pub title: String,
	#[serde(default)]
	pub cover_url: Option<String>,
	#[serde(default)]
	pub description: Option<String>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub is_nsfw: bool,
	#[serde(default)]
	pub authors: Vec<Named>,
	#[serde(default)]
	pub reading_mode: Option<String>,
	#[serde(default, rename = "type")]
	pub kind: Option<String>,
	#[serde(default)]
	pub genres: Vec<Named>,
	#[serde(default)]
	pub tags: Vec<Tag>,
}

#[derive(Deserialize)]
pub struct Named {
	pub name: String,
	#[serde(default)]
	pub role: Option<String>,
}

#[derive(Deserialize)]
pub struct Tag {
	pub name: String,
	#[serde(default)]
	pub is_spoiler: bool,
}

#[derive(Deserialize)]
pub struct ChapterListResponse {
	#[serde(default)]
	pub items: Vec<ChapterItem>,
	#[serde(default)]
	pub total: u32,
	#[serde(default)]
	pub limit: u32,
	#[serde(default)]
	pub offset: u32,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub number: f32,
	#[serde(default)]
	pub title: Option<String>,
	#[serde(default)]
	pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterBody {
	#[serde(default)]
	pub body: String,
	#[serde(default)]
	pub locked: bool,
}

#[derive(Deserialize)]
pub struct SeriesChapterBody {
	#[serde(default)]
	pub pages: Vec<String>,
}
