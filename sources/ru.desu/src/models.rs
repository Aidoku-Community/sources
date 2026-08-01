use crate::settings::eng_title;
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DesuError {
	pub message: Option<String>,
	pub code: Option<String>,
}

#[derive(Deserialize)]
pub struct DesuListResponse {
	pub mangas: Option<Vec<DesuItem>>,
	pub pagination: Option<DesuPagination>,
	pub errors: Option<Vec<DesuError>>,
}

#[derive(Deserialize)]
pub struct DesuPagination {
	pub current_page: Option<i32>,
	pub last_page: Option<i32>,
}

#[derive(Deserialize)]
pub struct DesuMangaResponse {
	pub manga: Option<DesuItem>,
	pub errors: Option<Vec<DesuError>>,
}

#[derive(Deserialize)]
pub struct DesuChaptersResponse {
	pub chapters: Option<Vec<DesuChapter>>,
	pub errors: Option<Vec<DesuError>>,
}

#[derive(Deserialize)]
pub struct DesuChapterResponse {
	pub chapter: Option<DesuChapterDetails>,
	pub errors: Option<Vec<DesuError>>,
}

#[derive(Deserialize)]
pub struct DesuCover {
	pub preview: Option<String>,
	pub snippet: Option<String>,
	pub x120: Option<String>,
}

#[derive(Deserialize)]
pub struct DesuGenre {
	pub name: String,
}

#[derive(Deserialize)]
pub struct DesuAuthor {
	pub name: String,
}

#[derive(Deserialize, Clone)]
pub struct DesuChapter {
	pub chapter_id: i64,
	pub volume: Option<String>,
	pub number: Option<String>,
	pub title: Option<String>,
	pub publish_date: Option<i64>,
	pub view_url: Option<String>,
}

#[derive(Deserialize)]
pub struct DesuPage {
	pub url: Option<String>,
}

#[derive(Deserialize)]
pub struct DesuChapterDetails {
	pub pages: Option<Vec<DesuPage>>,
}

#[derive(Deserialize)]
pub struct DesuItem {
	pub manga_id: i64,
	pub name: String,
	pub russian: Option<String>,
	pub cover: Option<DesuCover>,
	pub kind: Option<String>,
	pub reading_direction: Option<String>,
	pub recommended_reading_mode: Option<String>,
	pub age_limit: Option<String>,
	pub status: Option<String>,
	pub translation_status: Option<String>,
	pub description: Option<String>,
	pub view_url: Option<String>,
	pub genres: Option<Vec<DesuGenre>>,
	pub authors: Option<Vec<DesuAuthor>>,
}

fn parse_f32(value: Option<&String>) -> Option<f32> {
	value.and_then(|v| v.parse().ok())
}

impl From<DesuChapter> for Chapter {
	fn from(value: DesuChapter) -> Self {
		Self {
			key: value.chapter_id.to_string(),
			volume_number: parse_f32(value.volume.as_ref()),
			chapter_number: parse_f32(value.number.as_ref()),
			title: value.title,
			date_uploaded: value.publish_date,
			url: value.view_url,
			..Default::default()
		}
	}
}

impl DesuItem {
	pub fn into_manga(self, manga: Option<Manga>, slim: bool, details: bool) -> Manga {
		let mut item = manga.unwrap_or(Manga {
			key: self.manga_id.to_string(),
			..Default::default()
		});

		item.title = if eng_title() {
			self.name
		} else {
			self.russian.unwrap_or(self.name)
		};

		item.cover = self.cover.and_then(|v| v.preview.or(v.snippet).or(v.x120));

		if slim {
			return item;
		}

		if details {
			item.content_rating = self
				.age_limit
				.map(|v| match v.as_str() {
					"18_plus" => ContentRating::NSFW,
					"16_plus" => ContentRating::Suggestive,
					_ => ContentRating::Safe,
				})
				.unwrap_or_default();

			item.status = self
				.translation_status
				.as_deref()
				.and_then(|v| match v {
					"continued" => Some(MangaStatus::Ongoing),
					"completed" => Some(MangaStatus::Completed),
					_ => None,
				})
				.or_else(|| {
					self.status.as_deref().map(|v| match v {
						"ongoing" => MangaStatus::Ongoing,
						"released" => MangaStatus::Completed,
						_ => MangaStatus::Unknown,
					})
				})
				.unwrap_or_default();

			let kind = self.kind.as_deref().unwrap_or("");
			item.viewer = match kind {
				"manhwa" | "manhua" => Viewer::Webtoon,
				_ => match self.recommended_reading_mode.as_deref() {
					Some("vertical") => Viewer::Webtoon,
					_ => match self.reading_direction.as_deref() {
						Some("left-to-right") => Viewer::LeftToRight,
						Some("top-to-bottom") => Viewer::Webtoon,
						_ => Viewer::RightToLeft,
					},
				},
			};

			item.authors = self
				.authors
				.map(|l| l.into_iter().map(|v| v.name).collect());

			item.description = self.description;
			item.url = self.view_url;
			item.tags = self.genres.map(|l| l.into_iter().map(|v| v.name).collect());
		}

		item
	}
}
