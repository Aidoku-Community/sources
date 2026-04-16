use aidoku::alloc::{String, Vec};

#[derive(serde::Deserialize)]
pub struct ListComicResponse {
	pub comics: Vec<ListComic>,
}

#[derive(serde::Deserialize)]
pub struct ListComic {
	pub title: String,
	pub thumbnail: String,
	pub author: Option<String>,
	pub artist: Option<String>,
	pub url: String,
	pub slug: String,
	pub updated_at: String,
	pub adult: i32,
	pub status: String,
	pub description: Option<String>,
	pub last_chapter: Option<ComicChapter>,
}

#[derive(serde::Deserialize)]
pub struct ComicDetailResponse {
	pub comic: ComicDetail,
}

#[derive(serde::Deserialize)]
pub struct ComicDetail {
	pub title: String,
	pub thumbnail: String,
	pub description: Option<String>,
	pub author: Option<String>,
	pub artist: Option<String>,
	pub adult: i32,
	pub status: String,
	pub url: String,
	pub chapters: Vec<ComicChapter>,
}

#[derive(serde::Deserialize)]
pub struct ComicChapter {
	pub full_title: Option<String>,
	pub title: Option<String>,
	pub chapter: Option<f64>,
	pub updated_at: Option<String>,
	pub published_on: Option<String>,
	pub url: String,
}

#[derive(serde::Deserialize)]
pub struct ReadResponse {
	pub chapter: ReadChapter,
}

#[derive(serde::Deserialize)]
pub struct ReadChapter {
	pub pages: Vec<String>,
}
