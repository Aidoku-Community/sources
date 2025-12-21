use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
};
use serde::{self, Deserialize};

use crate::{BASE_URL, settings};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComixResponse<T> {
	pub status: i64,
	pub result: T,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultData<T> {
	pub items: Vec<T>,
	pub pagination: Pagination,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComixManga<'a> {
	pub manga_id: i64,
	pub hash_id: &'a str,
	pub title: Option<String>,
	pub alt_titles: Vec<String>,
	pub synopsis: Option<String>,
	pub slug: &'a str,
	pub rank: i64,
	#[serde(rename = "type")]
	pub type_: ComixTypeFilter,

	pub poster: Poster<'a>,

	pub original_language: Option<String>,
	pub status: ComixStatus,

	pub final_volume: f64,
	pub final_chapter: f64,

	pub has_chapters: bool,
	pub latest_chapter: f64,

	pub chapter_updated_at: i64,

	// pub start_date: i64,
	// // pub end_date: StringOrNumber,
	// pub created_at: i64,
	// pub updated_at: i64,
	pub rated_avg: f64,
	pub rated_count: i64,
	pub follows_total: i64,

	pub links: Links,

	pub is_nsfw: bool,

	pub year: i64,
	pub term_ids: Vec<i64>,
	pub demographic: Option<Vec<ComixBaseItem>>,
	pub genre: Option<Vec<ComixBaseItem>>,
	pub author: Option<Vec<ComixBaseItem>>,
	pub theme: Option<Vec<ComixBaseItem>>,
	pub artist: Option<Vec<ComixBaseItem>>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ComixBaseItem {
	pub term_id: i64,
	pub title: String,
	#[serde(rename = "type")]
	pub type_: String,
	pub slug: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ComixChapter {
	pub chapter_id: i64,
	pub manga_id: i64,

	pub scanlation_group_id: i64,

	pub is_official: i64,

	pub number: f64,
	pub name: String,
	pub language: String,

	pub volume: i64,
	pub votes: i64,

	pub created_at: i64,
	pub updated_at: i64,

	pub scanlation_group: Option<ScanlationGroup>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ChapterResponse {
	pub status: i64,
	pub result: Option<Item>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct Item {
	pub chapter_id: i64,
	pub images: Vec<Images>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct Images {
	pub url: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ScanlationGroup {
	pub scanlation_group_id: i64,
	pub name: String,
	pub slug: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Poster<'a> {
	pub small: &'a str,
	pub medium: &'a str,
	pub large: &'a str,
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

impl ComixManga<'_> {
	pub fn into_basic_manga(self) -> Manga {
		Manga {
			key: String::from(self.hash_id),
			title: self.title().unwrap_or_default(),
			cover: self.cover(),
			..Default::default()
		}
	}

	pub fn title(&self) -> Option<String> {
		self.title.clone()
	}

	pub fn description(&self) -> Option<String> {
		self.synopsis.clone()
	}

	pub fn cover(&self) -> Option<String> {
		let thumbnail_quality = settings::get_image_quality();
		match thumbnail_quality.as_str() {
			"small" => Some(self.poster.small.into()),
			"medium" => Some(self.poster.medium.into()),
			"large" => Some(self.poster.large.into()),
			_ => None,
		}
	}

	pub fn authors(&self) -> Vec<String> {
		self.author
			.as_ref()
			.filter(|authors| !authors.is_empty())
			.map(|authors| authors.iter().map(|a| a.title.clone()).collect())
			.unwrap_or_default()
	}

	pub fn artists(&self) -> Vec<String> {
		self.artist
			.as_ref()
			.filter(|artist| !artist.is_empty())
			.map(|artist| artist.iter().map(|a| a.title.clone()).collect())
			.unwrap_or_default()
	}

	pub fn url(&self) -> String {
		format!("{BASE_URL}/title/{}", self.hash_id)
	}

	pub fn tags(&self) -> Vec<String> {
		self.genre
			.as_ref()
			.filter(|gerne| !gerne.is_empty())
			.map(|gerne| gerne.iter().map(|g| g.title.clone()).collect())
			.unwrap_or_default()
	}

	pub fn status(&self) -> MangaStatus {
		match self.status {
			ComixStatus::Releasing => MangaStatus::Ongoing,
			ComixStatus::Finished => MangaStatus::Completed,
			ComixStatus::OnHiatus => MangaStatus::Hiatus,
			ComixStatus::Discontinued => MangaStatus::Cancelled,
			ComixStatus::NotYetReleased => MangaStatus::Unknown,
		}
	}

	pub fn content_rating(&self) -> ContentRating {
		if self.is_nsfw {
			ContentRating::NSFW
		} else {
			ContentRating::Safe
		}
	}
}

impl From<ComixManga<'_>> for Manga {
	fn from(val: ComixManga<'_>) -> Self {
		let tags = val.tags();
		let viewer = match val.type_ {
			ComixTypeFilter::Manga => Viewer::RightToLeft,
			ComixTypeFilter::Manhwa => Viewer::Webtoon,
			ComixTypeFilter::Manhua => Viewer::Webtoon,
			_ => Viewer::RightToLeft,
		};

		Manga {
			key: String::from(val.hash_id),
			title: val.title().unwrap_or_default(),
			cover: val.cover(),
			artists: Some(val.artists()),
			authors: Some(val.authors()),
			description: val.description(),
			url: Some(val.url()),
			tags: Some(tags),
			status: val.status(),
			content_rating: val.content_rating(),
			viewer,
			..Default::default()
		}
	}
}

impl ComixChapter {
	pub fn has_external_url(&self) -> bool {
		false
	}

	pub fn url(&self, manga: &Manga) -> String {
		match manga.url.as_deref() {
			Some(base) => format!("{}/{}", base, self.chapter_id),
			None => String::new(),
		}
	}

	pub fn manga_id(&self) -> Option<i64> {
		self.chapter_id.into()
	}

	pub fn scanlators(&self) -> Vec<String> {
		match self.scanlation_group.as_ref() {
			Some(group) => vec![group.name.clone()],
			None => Vec::new(),
		}
	}
}

impl From<ComixChapter> for Chapter {
	fn from(val: ComixChapter) -> Self {
		let chapter_number = Some(val.number as f32);
		let volume_number = Some(val.volume as f32);

		let title = if (volume_number == Some(0.0)) && val.name.is_empty() {
			Some("".into())
		} else {
			Some(val.name.clone())
		};

		Chapter {
			key: String::from(val.chapter_id.to_string()),
			title,
			chapter_number,
			volume_number,
			date_uploaded: Some(val.updated_at),
			scanlators: Some(val.scanlators()),
			// url: Some(val.url),
			language: Some(String::from(val.language)),
			..Default::default()
		}
	}
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ComixStatus {
	Releasing,
	#[default]
	Finished,
	OnHiatus,
	Discontinued,
	NotYetReleased,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ComixTypeFilter {
	#[default]
	Manga,
	Manhwa,
	Manhua,
	Other,
}
