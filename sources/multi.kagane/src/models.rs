use aidoku::{
	Manga,
	alloc::{format, string::String, vec::Vec},
};
use serde::Deserialize;

use crate::{API_BASE, BASE_URL};

fn default_true() -> bool {
	true
}

// ── Search ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchResponse {
	#[serde(default)]
	pub content: Vec<SearchItem>,
	#[serde(default = "default_true")]
	pub last: bool,
}

#[derive(Deserialize)]
pub struct SearchItem {
	pub series_id: String,
	pub title: String,
	pub cover_image_id: Option<String>,
	#[serde(default)]
	pub latest_chapters: Vec<LatestChapter>,
}

#[derive(Deserialize)]
pub struct LatestChapter {
	pub book_id: String,
	pub title: Option<String>,
	pub chapter_no: Option<String>,
	pub volume_no: Option<String>,
	pub created_at: Option<String>,
}

impl From<SearchItem> for Manga {
	fn from(s: SearchItem) -> Self {
		let url = Some(format!("{BASE_URL}/series/{}", s.series_id));
		Manga {
			key: s.series_id,
			title: String::from(s.title.trim()),
			cover: s.cover_image_id.map(|id| format!("{API_BASE}/image/{id}")),
			url,
			..Default::default()
		}
	}
}

// ── Series detail ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SeriesDetail {
	pub title: String,
	pub description: Option<String>,
	pub upload_status: String,
	pub format: Option<String>,
	pub content_rating: Option<String>,
	#[serde(default)]
	pub series_staff: Vec<StaffMember>,
	#[serde(default)]
	pub genres: Vec<GenreItem>,
	#[serde(default)]
	pub series_books: Vec<BookItem>,
	#[serde(default)]
	pub series_covers: Vec<CoverItem>,
}

#[derive(Deserialize)]
pub struct StaffMember {
	pub name: String,
	pub role: String,
}

#[derive(Deserialize)]
pub struct GenreItem {
	pub genre_name: String,
}

#[derive(Deserialize)]
pub struct BookItem {
	pub book_id: String,
	pub title: String,
	pub created_at: Option<String>,
	pub chapter_no: Option<String>,
	pub volume_no: Option<String>,
	#[serde(default)]
	pub groups: Vec<GroupItem>,
}

#[derive(Deserialize)]
pub struct GroupItem {
	pub title: String,
}

#[derive(Deserialize)]
pub struct CoverItem {
	pub image_id: String,
}

// ── Page listing (DRM) ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct IntegrityResponse {
	pub token: String,
}

#[derive(Deserialize)]
pub struct ChallengeResponse {
	pub access_token: String,
	pub cache_url: String,
	pub manifest: Option<ManifestData>,
}

#[derive(Deserialize)]
pub struct ManifestData {
	#[serde(default)]
	pub pages: Vec<PageData>,
}

#[derive(Deserialize)]
pub struct PageData {
	pub page_no: i32,
	pub page_id: String,
	pub ext: Option<String>,
}
