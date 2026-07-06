use aidoku::{
	Chapter, Manga, Page, PageContent,
	alloc::{String, Vec, format, string::ToString},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApiResponse<T> {
	pub items: Vec<T>,
	pub meta: Option<ApiMeta>,
}

#[derive(Deserialize)]
pub struct ApiMeta {
	#[serde(rename = "hasNext")]
	pub has_next: bool,
}

#[derive(Deserialize)]
pub struct ApiManga {
	pub hid: String,
	pub slug: Option<String>,
	pub title: String,
	pub poster: Option<ApiPoster>,
	#[serde(rename = "latestChapter")]
	pub latest_chapter: Option<f32>,
}

impl From<ApiManga> for Manga {
	fn from(value: ApiManga) -> Self {
		Manga {
			key: value.hid,
			title: value.title,
			cover: value
				.poster
				.and_then(|poster| poster.large.or(poster.medium).or(poster.small)),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ApiPoster {
	pub small: Option<String>,
	pub medium: Option<String>,
	pub large: Option<String>,
}

#[derive(Deserialize)]
pub struct ApiDetailsResponse {
	pub data: ApiMangaDetails,
}

#[derive(Deserialize)]
pub struct ApiMangaDetails {
	// pub hid: String,
	// pub slug: Option<String>,
	pub title: String,
	#[serde(rename = "type")]
	pub manga_type: Option<String>,
	pub status: Option<String>,
	pub poster: Option<ApiPoster>,
	#[serde(rename = "synopsisHtml")]
	pub synopsis_html: Option<String>,
	pub authors: Option<Vec<ApiEntity>>,
	pub artists: Option<Vec<ApiEntity>>,
	pub genres: Option<Vec<ApiEntity>>,
	pub themes: Option<Vec<ApiEntity>>,
}

#[derive(Deserialize)]
pub struct ApiEntity {
	pub title: String,
}

#[derive(Deserialize)]
pub struct ApiChapter {
	pub id: i32,
	pub number: f32,
	pub name: Option<String>,
	#[serde(rename = "createdAt")]
	pub created_at: Option<i64>,
}

impl ApiChapter {
	pub fn into_chapter(self, manga_key: &str, lang: &str) -> Chapter {
		Chapter {
			key: self.id.to_string(),
			title: self.name.filter(|name| !name.is_empty()),
			chapter_number: Some(self.number),
			date_uploaded: self.created_at,
			url: Some(format!("{manga_key}/{}", self.id)),
			language: Some(lang.to_string()),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ApiPagesResponse {
	pub data: ApiChapterPages,
}

#[derive(Deserialize)]
pub struct ApiChapterPages {
	pub pages: Vec<ApiPage>,
}

#[derive(Deserialize)]
pub struct ApiPage {
	pub url: String,
}

impl From<ApiPage> for Page {
	fn from(value: ApiPage) -> Self {
		Page {
			content: PageContent::url(value.url),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ApiTagsResponse {
	pub data: Vec<ApiTag>,
}

#[derive(Deserialize)]
pub struct ApiTag {
	pub id: i32,
	pub name: String,
	#[serde(rename = "type")]
	pub tag_type: String,
}
