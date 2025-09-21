use aidoku::{
	HashMap,
	alloc::{String, Vec},
};
use serde::Deserialize;

#[derive(Default, Deserialize, Debug, Clone)]
pub struct RecoEntry {
	pub file: String,
	// pub color: String,
}

pub type RecoFile = Vec<RecoEntry>;

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ImageData {
	// pub id: String,
	// pub description: Option<String>,
	pub link: String,
	pub thumbnail: String,
	// pub mp4: i32,
	// pub position: i32,
	// pub type: String,
	// pub size: i64,
	// pub width: i32,
	// pub height: i32,
	// pub nsfw: Option<bool>,
}

pub type ImageList = Vec<ImageData>;

#[derive(Default, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct ConfigJson {
	pub LOCAL_SERIES_FILES: Vec<String>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct SeriesData {
	pub title: String,
	pub description: String,
	pub artist: String,
	pub author: String,
	pub cover_hq: String,
	pub cover_low: String,
	pub tags: Vec<String>,
	pub release_status: String,
	pub covers_gallery: Option<Vec<CoverEntry>>,
	pub chapters: HashMap<String, ChapterData>,
	pub os: Option<bool>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct CoverEntry {
	pub url_hq: String,
	// pub url_lq: String,
	// pub volume: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ChapterData {
	pub title: String,
	pub volume: String,
	pub last_updated: String,
	pub licencied: Option<bool>,
	pub groups: HashMap<String, String>,
	pub collab: Option<String>,
}
