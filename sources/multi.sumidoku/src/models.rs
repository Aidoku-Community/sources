use aidoku::{
	Chapter, ContentRating, HashMap, Manga, MangaStatus, Page, PageContent, UpdateStrategy, Viewer,
	alloc::{String, Vec, format},
};
use alloc::string::ToString;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
	pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SumiCategory {
	pub id: i64,
	pub name: String,
	pub manga_count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
	pub entries: Vec<LibraryEntry>,
	pub has_next_page: bool,
}

#[derive(Debug, Deserialize)]
pub struct LibraryEntry {
	#[serde(flatten)]
	pub manga: SumiManga,
}

#[derive(Debug, Deserialize)]
pub struct SumiSource {
	pub id: i64,
	pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCount {
	pub source_id: i64,
}

// Note: manga fields are snake_case (SourceManga schema), unlike the
// camelCase used by server-native endpoints.
#[derive(Debug, Deserialize)]
pub struct SumiManga {
	pub id: i64,
	pub title: String,
	pub artists: Option<Vec<String>>,
	pub authors: Option<Vec<String>>,
	pub description: Option<String>,
	pub tags: Option<Vec<String>>,
	pub status: Option<String>,
	pub content_rating: Option<String>,
	pub viewer: Option<String>,
	pub update_strategy: Option<String>,
	pub next_update_time: Option<i64>,
}

impl SumiManga {
	pub fn into_listing(self, base_url: &str) -> Manga {
		Manga {
			key: self.id.to_string(),
			title: self.title,
			cover: Some(cover_url(self.id, base_url)),
			..Default::default()
		}
	}

	pub fn into_manga(self, base_url: &str) -> Manga {
		Manga {
			key: self.id.to_string(),
			title: self.title,
			cover: Some(cover_url(self.id, base_url)),
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
pub struct SumiChapter {
	pub id: i64,
	pub title: Option<String>,
	pub chapter_number: Option<f32>,
	pub volume_number: Option<f32>,
	pub date_uploaded: Option<i64>,
	pub scanlators: Option<Vec<String>>,
	pub language: Option<String>,
	pub thumbnail: Option<String>,
	pub locked: bool,
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
#[serde(rename_all = "camelCase")]
pub struct SumiPage {
	pub content: SumiPageContent,
	pub thumbnail: Option<String>,
	pub has_description: bool,
	pub description: Option<String>,
	pub index: i64,
}

#[derive(Debug, Deserialize)]
pub enum SumiPageContent {
	Url(String, Option<HashMap<String, String>>),
	Text(String),
	// Image/Zip/Local/OpdsZip/Downloaded pages are served by the server's page endpoint
	#[serde(other)]
	Other,
}

impl SumiPage {
	pub fn into_page(self, base_url: &str, chapter_id: i64) -> Page {
		let content = match self.content {
			SumiPageContent::Url(url, headers) => {
				PageContent::Url(absolute_url(base_url, &url), headers)
			}
			SumiPageContent::Text(text) => PageContent::Text(text),
			SumiPageContent::Other => {
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

fn cover_url(manga_id: i64, base_url: &str) -> String {
	format!("{base_url}/api/v1/manga/{manga_id}/cover")
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
