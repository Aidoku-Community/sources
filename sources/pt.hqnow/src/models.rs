use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec},
};
use alloc::string::ToString;
use serde::Deserialize;

// ── Response wrapper ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GqlResponse<T> {
	pub data: Option<T>,
}

// ── Response data shapes ──────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByNameResponse {
	#[serde(default)]
	pub get_hqs_by_name: Vec<HqBasic>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByFiltersResponse {
	#[serde(default)]
	pub get_hqs_by_filters: Vec<HqBasic>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByIdResponse {
	#[serde(default)]
	pub get_hqs_by_id: Vec<HqDetail>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterByIdResponse {
	pub get_chapter_by_id: Option<ChapterPages>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentResponse {
	#[serde(default)]
	pub get_recently_updated_hqs: Vec<HqBasic>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarouselResponse {
	#[serde(default)]
	pub get_carousel_of_hqs: Vec<CarouselItem>,
}

// ── Domain types ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HqBasic {
	pub id: i32,
	pub name: String,
	pub hq_cover: Option<String>,
	pub synopsis: Option<String>,
}

impl HqBasic {
	pub fn into_manga(self) -> Manga {
		Manga {
			key: self.id.to_string(),
			title: self.name,
			cover: to_https(self.hq_cover),
			description: self.synopsis,
			content_rating: ContentRating::Safe,
			viewer: Viewer::LeftToRight,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HqDetail {
	pub id: i32,
	pub name: String,
	pub synopsis: Option<String>,
	pub hq_cover: Option<String>,
	pub publisher_name: Option<String>,
	pub status: Option<String>,
	#[serde(default)]
	pub capitulos: Vec<HqChapter>,
}

#[derive(Deserialize)]
pub struct HqChapter {
	pub id: i32,
	pub name: Option<String>,
	pub number: String,
}

impl HqChapter {
	pub fn into_chapter(self) -> Chapter {
		Chapter {
			key: self.id.to_string(),
			title: self.name.filter(|n| !n.is_empty()),
			chapter_number: self.number.parse().ok(),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterPages {
	#[serde(default)]
	pub pictures: Vec<HqPage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HqPage {
	pub picture_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarouselItem {
	pub hq_id: i32,
	pub name: String,
	pub hq_cover: Option<String>,
}

impl CarouselItem {
	pub fn into_manga(self) -> Manga {
		Manga {
			key: self.hq_id.to_string(),
			title: self.name,
			cover: to_https(self.hq_cover),
			content_rating: ContentRating::Safe,
			viewer: Viewer::LeftToRight,
			..Default::default()
		}
	}
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub fn parse_status(s: Option<&str>) -> MangaStatus {
	match s {
		Some("Concluído") => MangaStatus::Completed,
		Some("Em Andamento") => MangaStatus::Ongoing,
		Some("Cancelado") => MangaStatus::Cancelled,
		Some("Hiatus") => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

/// Rewrites `http://` image URLs to `https://` to prevent mixed-content blocks.
pub fn to_https(url: Option<String>) -> Option<String> {
	url.map(|u| {
		if u.starts_with("http://") {
			u.replacen("http://", "https://", 1)
		} else {
			u
		}
	})
}
