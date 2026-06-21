use aidoku::alloc::{string::String, vec::Vec};
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

#[derive(Deserialize)]
pub struct ApiChapter {
    pub slug: String,
    pub number: f64,
    pub title: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Deserialize)]
pub struct ApiChapterDetail {
    pub images: Vec<ApiImage>,
}

#[derive(Deserialize)]
pub struct ApiImage {
    pub url: String,
}
