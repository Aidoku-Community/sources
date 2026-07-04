use aidoku::{
	Chapter, Manga, MangaWithChapter,
	alloc::{format, string::String, vec::Vec},
	imports::std::parse_date,
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

/// Pair a series with its most recent chapter, as returned in the search
/// endpoint's `latest_chapters` field. Fails when the series has no chapters,
/// so it can be dropped from home-feed listings via `filter_map(.. .ok())`.
impl TryFrom<SearchItem> for MangaWithChapter {
	type Error = ();

	fn try_from(mut item: SearchItem) -> Result<Self, Self::Error> {
		let book = item.latest_chapters.drain(..).next().ok_or(())?;
		let manga = Manga::from(item);
		Ok(MangaWithChapter {
			chapter: Chapter {
				key: book.book_id,
				chapter_number: book.chapter_no.as_deref().and_then(|s| s.parse().ok()),
				volume_number: book.volume_no.as_deref().and_then(|s| s.parse().ok()),
				title: book.title.and_then(|t| {
					let t = t.trim();
					if t.is_empty() {
						None
					} else {
						Some(String::from(t))
					}
				}),
				date_uploaded: book.created_at.as_deref().and_then(|s| {
					let s = s.split_once('.').map_or(s, |(b, _)| b);
					parse_date(format!("{s}Z"), "yyyy-MM-dd'T'HH:mm:ss'Z'")
				}),
				..Default::default()
			},
			manga,
		})
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
	pub tags: Vec<TagItem>,
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
	#[serde(default)]
	pub is_spoiler: bool,
}

#[derive(Deserialize)]
pub struct TagItem {
	pub tag_name: String,
	#[serde(default)]
	pub is_spoiler: bool,
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
