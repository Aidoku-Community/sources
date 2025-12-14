// use crate::Params;
use aidoku::{
	Chapter, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::element::ElementHelpers,
	imports::html::Html,
	prelude::*,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
// #[serde(bound(deserialize = "'de: 'a"))]
pub struct ApiResponse {
	pub status: i64,
	pub result: ResultData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultData {
	pub items: Vec<MangaItem>,
	pub pagination: Pagination,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MangaItem {
	pub manga_id: i64,
	pub hash_id: String,
	pub title: String,
	pub alt_titles: Vec<String>,
	pub synopsis: String,
	pub slug: String,
	pub rank: i64,
	#[serde(rename = "type")]
	pub type_: String,

	pub poster: Poster,

	pub original_language: Option<String>,
	pub status: String,

	pub final_volume: i64,
	pub final_chapter: i64,

	pub has_chapters: bool,
	pub latest_chapter: f64,

	pub chapter_updated_at: i64,

	pub start_date: i64,
	pub end_date: String,

	pub created_at: i64,
	pub updated_at: i64,

	pub rated_avg: f64,
	pub rated_count: i64,
	pub follows_total: i64,

	pub links: Links,

	pub is_nsfw: bool,

	pub year: i64,
	pub term_ids: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Poster {
	pub small: String,
	pub medium: String,
	pub large: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Links {
	pub al: Option<String>,
	pub mal: Option<String>,
	pub mu: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
	pub count: i64,
	pub total: i64,
	pub per_page: i64,
	pub current_page: i64,
	pub last_page: i64,
	pub from: i64,
	pub to: i64,
}
