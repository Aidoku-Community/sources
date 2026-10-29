use crate::Params;
use aidoku::{
	Chapter, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, vec},
	helpers::element::ElementHelpers,
	imports::{html::Html, std::parse_date},
	prelude::*,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Paginated<T> {
	pub data: Vec<T>,
	pub meta: Meta,
}

#[derive(Deserialize)]
pub struct Meta {
	current_page: i32,
	last_page: i32,
}

impl Meta {
	pub fn has_next_page(&self) -> bool {
		self.current_page < self.last_page
	}
}

#[derive(Deserialize)]
pub struct Tag {
	pub id: i32,
	pub name: String,
}

#[derive(Deserialize)]
pub struct Series {
	pub id: i32,
	series_slug: String,
	title: String,
	thumbnail: Option<String>,
	description: Option<String>,
	author: Option<String>,
	studio: Option<String>,
	status: Option<String>,
	tags: Option<Vec<Tag>>,
}

#[derive(Deserialize)]
pub struct HeanChapter {
	chapter_name: String,
	chapter_title: Option<String>,
	chapter_slug: String,
	chapter_thumbnail: Option<String>,
	created_at: Option<String>,
	price: Option<i32>,
}

#[derive(Deserialize)]
pub struct ChapterResponse {
	pub chapter: ChapterDetails,
}

#[derive(Deserialize)]
pub struct ChapterDetails {
	pub chapter_data: ChapterData,
}

#[derive(Deserialize)]
pub struct ChapterData {
	pub images: Option<Vec<String>>,
}

fn non_empty(value: Option<String>) -> Option<Vec<String>> {
	value
		.map(|s| s.trim().into())
		.filter(|s: &String| !s.is_empty())
		.map(|s| vec![s])
}

impl Series {
	pub fn into_manga(self, params: &Params) -> Manga {
		Manga {
			key: self.key(),
			title: self.title,
			cover: self.thumbnail.filter(|t| !t.is_empty()),
			authors: non_empty(self.author),
			artists: non_empty(self.studio),
			description: self
				.description
				.and_then(|d| Html::parse_fragment(d).ok())
				.and_then(|html| html.select_first("body")?.text_with_newlines())
				.map(|d| d.trim().into()),
			url: Some(format!("{}/series/{}", params.base_url, self.series_slug)),
			tags: self
				.tags
				.map(|tags| tags.into_iter().map(|t| t.name).collect()),
			status: match self.status.as_deref() {
				Some("Ongoing") => MangaStatus::Ongoing,
				Some("Hiatus") => MangaStatus::Hiatus,
				Some("Dropped" | "Canceled") => MangaStatus::Cancelled,
				Some("Completed" | "Finished") => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			},
			viewer: Viewer::Webtoon,
			..Default::default()
		}
	}

	pub fn into_basic_manga(self) -> Manga {
		Manga {
			key: self.key(),
			title: self.title,
			cover: self.thumbnail.filter(|t| !t.is_empty()),
			..Default::default()
		}
	}

	pub fn key(&self) -> String {
		format!("{}|{}", self.series_slug, self.id)
	}
}

pub fn split_key(key: &str) -> (&str, Option<i32>) {
	match key.split_once('|') {
		Some((slug, id)) => (slug, id.parse().ok()),
		None => (key, None),
	}
}

impl HeanChapter {
	pub fn into_chapter(self, base_url: &str, series_slug: &str) -> Chapter {
		let chapter_number = self
			.chapter_name
			.split_whitespace()
			.find_map(|s| s.parse().ok());
		Chapter {
			url: Some(format!(
				"{base_url}/series/{series_slug}/{}",
				self.chapter_slug
			)),
			key: self.chapter_slug,
			title: self
				.chapter_title
				.filter(|t| !t.trim().is_empty())
				.or_else(|| chapter_number.is_none().then_some(self.chapter_name)),
			chapter_number,
			date_uploaded: self
				.created_at
				.and_then(|d| parse_date(d, "yyyy-MM-dd'T'HH:mm:ss.SSSXXX")),
			thumbnail: self.chapter_thumbnail.filter(|t| !t.is_empty()),
			locked: self.price.is_some_and(|p| p > 0),
			..Default::default()
		}
	}
}
