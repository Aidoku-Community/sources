use aidoku::{
	Chapter, ContentRating, HashMap, Manga, MangaStatus, Page, PageContent, UpdateStrategy, Viewer,
	alloc::{String, Vec, format},
};
use alloc::string::ToString;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
	pub token: String,
	pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
pub struct SumiCategory {
	pub id: i64,
	pub name: String,
	pub sort_order: i64,
	pub is_default: bool,
	pub manga_count: i64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
	pub entries: Vec<LibraryEntry>,
	pub has_next_page: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LibraryEntry {
	#[serde(flatten)]
	pub manga: SumiManga,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SumiSource {
	pub id: i64,
	pub name: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
pub struct SourceCount {
	pub source_id: i64,
	pub count: i64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
pub struct SumiManga {
	pub id: i64,
	pub title: String,
	#[serde(default)]
	pub source_id: i64,
	#[serde(default)]
	pub key: String,
	pub cover: Option<String>,
	pub artists: Option<Vec<String>>,
	pub authors: Option<Vec<String>>,
	pub description: Option<String>,
	pub url: Option<String>,
	pub tags: Option<Vec<String>>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub content_rating: Option<String>,
	#[serde(default)]
	pub viewer: Option<String>,
	#[serde(default)]
	pub update_strategy: Option<String>,
	pub next_update_time: Option<i64>,
	#[serde(default)]
	pub chapter_count: i64,
	#[serde(default)]
	pub in_library: bool,
	#[serde(default)]
	pub category_ids: Vec<i64>,
}

impl SumiManga {
	pub fn into_listing(self, base_url: &str) -> Manga {
		Manga {
			key: self.id.to_string(),
			title: self.title,
			cover: Some(cover_url(self.cover, self.id, base_url)),
			..Default::default()
		}
	}

	pub fn into_manga(self, base_url: &str) -> Manga {
		Manga {
			key: self.id.to_string(),
			title: self.title,
			cover: Some(cover_url(self.cover, self.id, base_url)),
			artists: self.artists,
			authors: self.authors,
			description: self.description,
			url: Some(format!("{base_url}/manga/{}", self.id)),
			tags: self.tags,
			status: self.status.as_deref().map(parse_status).unwrap_or_default(),
			content_rating: self
				.content_rating
				.as_deref()
				.map(parse_content_rating)
				.unwrap_or_default(),
			viewer: self.viewer.as_deref().map(parse_viewer).unwrap_or_default(),
			update_strategy: self
				.update_strategy
				.as_deref()
				.map(parse_update_strategy)
				.unwrap_or_default(),
			next_update_time: self.next_update_time,
			chapters: None,
		}
	}
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
pub struct SumiChapter {
	pub id: i64,
	#[serde(default)]
	pub key: String,
	pub title: Option<String>,
	pub chapter_number: Option<f32>,
	pub volume_number: Option<f32>,
	pub date_uploaded: Option<i64>,
	pub scanlators: Option<Vec<String>>,
	pub url: Option<String>,
	pub language: Option<String>,
	pub thumbnail: Option<String>,
	#[serde(default)]
	pub locked: bool,
	#[serde(default)]
	pub page_count: i64,
}

impl SumiChapter {
	pub fn into_chapter(self, base_url: &str) -> Chapter {
		Chapter {
			key: self.id.to_string(),
			title: self.title,
			chapter_number: self.chapter_number,
			volume_number: self.volume_number,
			date_uploaded: self.date_uploaded,
			scanlators: self.scanlators,
			url: Some(format!("{base_url}/reader/{}", self.id)),
			language: self.language,
			thumbnail: self.thumbnail.map(|t| absolute_url(base_url, &t)),
			locked: self.locked,
		}
	}
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
pub struct SumiPage {
	pub content: SumiPageContent,
	pub thumbnail: Option<String>,
	#[serde(default)]
	pub has_description: bool,
	pub description: Option<String>,
	pub index: i64,
	pub width: Option<i64>,
	pub height: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub enum SumiPageContent {
	Url(String, Option<HashMap<String, String>>),
	Text(String),
	Image(String),
	Zip(String, String),
	Local(String),
	OpdsZip(String),
	Downloaded(String),
}

impl SumiPage {
	pub fn into_page(self, base_url: &str, chapter_id: i64) -> Page {
		let content = match self.content {
			SumiPageContent::Url(url, headers) => {
				PageContent::Url(absolute_url(base_url, &url), headers)
			}
			SumiPageContent::Text(text) => PageContent::Text(text),
			_ => {
				let url = format!("{base_url}/api/v1/chapter/{chapter_id}/page/{}", self.index);
				PageContent::Url(url, None)
			}
		};

		Page {
			content,
			thumbnail: self.thumbnail.map(|t| absolute_url(base_url, &t)),
			has_description: self.has_description,
			description: self.description,
		}
	}
}

fn cover_url(cover: Option<String>, manga_id: i64, base_url: &str) -> String {
	match cover.filter(|c| !c.is_empty()) {
		Some(c) => absolute_url(base_url, &c),
		None => format!("{base_url}/api/v1/manga/{manga_id}/cover"),
	}
}

fn absolute_url(base_url: &str, url: &str) -> String {
	let trimmed = url.trim();
	if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
		trimmed.into()
	} else if trimmed.starts_with('/') {
		format!("{base_url}{trimmed}")
	} else {
		format!("{base_url}/{trimmed}")
	}
}

fn parse_status(value: &str) -> MangaStatus {
	match value {
		"Ongoing" => MangaStatus::Ongoing,
		"Completed" => MangaStatus::Completed,
		"Cancelled" => MangaStatus::Cancelled,
		"Hiatus" => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

fn parse_content_rating(value: &str) -> ContentRating {
	match value {
		"Safe" => ContentRating::Safe,
		"Suggestive" => ContentRating::Suggestive,
		"NSFW" => ContentRating::NSFW,
		_ => ContentRating::Unknown,
	}
}

fn parse_viewer(value: &str) -> Viewer {
	match value {
		"LeftToRight" => Viewer::LeftToRight,
		"RightToLeft" => Viewer::RightToLeft,
		"Vertical" => Viewer::Vertical,
		"Webtoon" => Viewer::Webtoon,
		_ => Viewer::Unknown,
	}
}

fn parse_update_strategy(value: &str) -> UpdateStrategy {
	match value {
		"Always" => UpdateStrategy::Always,
		"Never" => UpdateStrategy::Never,
		_ => UpdateStrategy::Always,
	}
}
