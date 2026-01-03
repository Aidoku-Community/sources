use crate::BASE_URL;
use aidoku::{Chapter, Manga, MangaWithChapter, alloc::*};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Related {
	pub data: Vec<Comic>,
}

#[derive(Deserialize)]
pub struct Top {
	pub data: TopData,
}

#[derive(Deserialize)]
pub struct TopData {
	pub top_all: Vec<Comic>,
	pub top_day: Vec<Comic>,
	pub top_month: Vec<Comic>,
	pub top_week: Vec<Comic>,
}

#[derive(Deserialize)]
pub struct Comic {
	pub name: String,
	pub slug: String,
	pub thumbnail: Option<String>,
	pub last_chapter: Option<VChapter>,
	pub genres: Vec<Genre>,
	pub r#type: String,
}

impl From<Comic> for Manga {
	fn from(value: Comic) -> Self {
		Self {
			url: Some(format!("{BASE_URL}/truyen-tranh/{}", value.slug)),
			key: value.slug,
			title: value.name,
			cover: value.thumbnail,
			artists: Some(vec![value.r#type]),
			tags: Some(value.genres.into_iter().map(|t| t.name).collect::<Vec<_>>()),
			chapters: value.last_chapter.map(|v| vec![v.into()]),
			..Default::default()
		}
	}
}

impl From<Comic> for MangaWithChapter {
	fn from(value: Comic) -> Self {
		Self {
			chapter: value.last_chapter.clone().unwrap_or_default().into(),
			manga: value.into(),
		}
	}
}

#[derive(Deserialize, Default, Clone)]
pub struct VChapter {
	pub chapter_name: Option<String>,
	pub chapter_num: Option<f32>,
	pub chapter_slug: String,
	pub updated_at: Option<DateTime<Utc>>,
}

impl From<VChapter> for Chapter {
	fn from(value: VChapter) -> Self {
		Self {
			key: value
				.chapter_num
				.map(|v| format!("chuong-{v}"))
				.unwrap_or(value.chapter_slug),
			title: value.chapter_name,
			chapter_number: value.chapter_num,
			date_uploaded: value.updated_at.map(|v| v.timestamp()),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct Chapters {
	pub chapters: Vec<VChapter>,
	pub current_page: usize,
	pub last_page: usize,
}

#[derive(Deserialize)]
pub struct ChaptersData {
	pub data: Chapters,
}

#[derive(Deserialize)]
pub struct Genre {
	pub name: String,
}
