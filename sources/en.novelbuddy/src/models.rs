use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApiResponse<T> {
	#[serde(default)]
	pub success: bool,
	pub message: Option<String>,
	pub data: Option<T>,
}

#[derive(Deserialize)]
pub struct ListData {
	pub items: Vec<TitleListItem>,
	pub pagination: Pagination,
}

#[derive(Deserialize)]
pub struct Pagination {
	#[serde(default)]
	pub has_next: bool,
}

#[derive(Deserialize)]
pub struct TrendingData {
	pub items: Vec<TitleListItem>,
}

#[derive(Deserialize)]
pub struct TitleListItem {
	pub id: String,
	pub name: String,
	#[serde(default)]
	pub slug: Option<String>,
	#[serde(default)]
	pub cover: Option<String>,
}

#[derive(Deserialize)]
pub struct TitleDetailData {
	pub title: TitleDetail,
}

#[derive(Deserialize)]
pub struct TitleDetail {
	pub id: String,
	pub name: String,
	#[serde(default)]
	pub slug: Option<String>,
	#[serde(default)]
	pub summary: Option<String>,
	#[serde(default)]
	pub cover: Option<String>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub genres: Vec<NamedSlug>,
	#[serde(default)]
	pub authors: Vec<NamedSlug>,
	#[serde(default)]
	pub artists: Vec<NamedSlug>,
	#[serde(default)]
	pub tags: Vec<NamedSlug>,
	#[serde(default)]
	pub is_adult: i32,
}

#[derive(Deserialize)]
pub struct NamedSlug {
	pub name: String,
}

#[derive(Deserialize)]
pub struct ChapterListData {
	pub chapters: Vec<ChapterListItem>,
}

#[derive(Deserialize)]
pub struct ChapterListItem {
	pub id: String,
	pub name: String,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub updated_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterDetailData {
	pub chapter: ChapterDetail,
}

#[derive(Deserialize)]
pub struct ChapterDetail {
	#[serde(default)]
	pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct BySlugData {
	pub new_url: String,
}
