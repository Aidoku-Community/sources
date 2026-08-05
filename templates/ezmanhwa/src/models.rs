use aidoku::{
	Manga,
	alloc::{format, string::String, vec::Vec},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApiList<T> {
	pub data: Vec<T>,
	pub next: Option<i32>,
}

#[derive(Deserialize)]
pub struct ApiSeriesItem {
	pub slug: String,
	pub title: String,
	pub cover: String,
	#[serde(rename = "type")]
	pub series_type: Option<String>,
	#[serde(default)]
	pub chapters: Vec<ApiHomeChapter>,
}

#[derive(Deserialize)]
pub struct ApiHomeChapter {
	pub slug: String,
	pub number: f64,
}

impl ApiSeriesItem {
	pub fn into_manga(self, base_url: &str) -> Manga {
		Manga {
			url: Some(format!("{base_url}/series/{}", self.slug)),
			key: self.slug,
			title: String::from(self.title.trim()),
			cover: if self.cover.is_empty() {
				None
			} else {
				Some(self.cover)
			},
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ApiSeriesDetail {
	pub slug: String,
	pub title: String,
	pub description: Option<String>,
	pub author: Option<String>,
	pub artist: Option<String>,
	pub cover: String,
	pub status: Option<String>,
	pub genres: Option<Vec<ApiGenre>>,
}

#[derive(Deserialize)]
pub struct ApiGenre {
	pub name: String,
}

fn default_true() -> bool {
	true
}

#[derive(Deserialize)]
pub struct ApiChapter {
	pub slug: String,
	pub number: f64,
	pub title: Option<String>,
	#[serde(rename = "createdAt")]
	pub created_at: Option<String>,
	#[serde(rename = "isFree", default = "default_true")]
	pub is_free: bool,
}

#[derive(Deserialize)]
pub struct ApiChapterDetail {
	pub images: Vec<ApiImage>,
}

#[derive(Deserialize)]
pub struct ApiImage {
	pub url: String,
}

#[derive(Deserialize)]
pub struct ApiHomeResponse {
	pub popular: Vec<ApiSeriesItem>,
	pub pinned: Vec<ApiSeriesItem>,
	#[serde(rename = "newSeries")]
	pub new_series: Vec<ApiSeriesItem>,
}
