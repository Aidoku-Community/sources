use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ListData {
	pub novels: Vec<Novel>,
	pub pagination: Pagination,
}

#[derive(Deserialize)]
pub struct ListingData {
	pub novels: Vec<Novel>,
}

#[derive(Deserialize)]
pub struct Pagination {
	#[serde(default)]
	pub has_next: bool,
}

#[derive(Deserialize)]
pub struct NovelDetailData {
	pub novel: Novel,
}

#[derive(Deserialize)]
pub struct Novel {
	pub id: String,
	pub title: String,
	#[serde(default)]
	pub author: Option<String>,
	#[serde(default)]
	pub description: Option<String>,
	#[serde(default)]
	pub genres: Option<String>,
	#[serde(default)]
	pub cover_url: Option<String>,
	#[serde(default)]
	pub image_url: Option<String>,
	#[serde(default)]
	pub novel_image: Option<String>,
	#[serde(default)]
	pub release_status: Option<String>,
	#[serde(default)]
	pub ongoing: Option<String>,
	#[serde(default)]
	pub chapter_names: Vec<String>,
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
