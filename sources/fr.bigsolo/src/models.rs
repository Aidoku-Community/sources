use aidoku::{
	HashMap, Manga, MangaStatus,
	alloc::{String, Vec, format, vec},
};
use serde::Deserialize;

use crate::BASE_URL;

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ChapterData {
	pub title: String,
	pub volume: Option<String>,
	pub timestamp: i64,
	pub licensed: Option<bool>,
	pub teams: Vec<String>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ChapterEndpointData {
	// contains ChapterData and the images.
	// we're only gonna use the images.
	pub images: Vec<String>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct Cover {
	pub url_hq: String,
	pub url_lq: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct Series {
	pub key: String,
	pub slug: String,
	pub title: String,
	pub description: String,
	pub artist: String,
	pub author: String,
	pub status: String,
	pub tags: Vec<String>,
	// pub ja_title: String,
	pub alternative_titles: Vec<String>,
	// pub release_rhythm: Option<String>,
	pub cover: Cover,
	pub covers: Vec<Cover>,
	pub os: Option<bool>,
	pub chapters: HashMap<String, ChapterData>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct SeriesList {
	pub series: Vec<Series>,
	pub os: Vec<Series>,
	pub reco: Vec<Series>,
}

pub fn map_bigsolo_status(status: &str) -> MangaStatus {
	match status {
		"En cours" => MangaStatus::Ongoing,
		"Fini" | "Finis" => MangaStatus::Completed,
		"En pause" => MangaStatus::Hiatus,
		"Annulé" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

impl From<Series> for Manga {
	fn from(series: Series) -> Self {
		let url = Some(format!("{BASE_URL}/{}", series.slug));

		Manga {
			key: series.slug,
			title: series.title,
			cover: Some(series.cover.url_lq),
			authors: Some(vec![series.author]),
			artists: Some(vec![series.artist]),
			description: Some(series.description),
			status: map_bigsolo_status(&series.status),
			tags: Some(series.tags),
			url,
			..Default::default()
		}
	}
}
