use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	helpers::element::ElementHelpers,
	imports::html::Html,
	prelude::*,
	Chapter, Manga, MangaStatus, Viewer,
};
use serde::Deserialize;

use crate::Params;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct SearchResponse<'a> {
	pub posts: Vec<Post<'a>>,
	pub total_count: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct PostResponse<'a> {
	pub post: Post<'a>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct ChaptersResponse<'a> {
	pub post: PostWithOnlyChapters<'a>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct ChapterResponse<'a> {
	pub chapter: IkenChapter<'a>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Post<'a> {
	pub id: i32,
	pub slug: &'a str,
	post_title: &'a str,
	post_content: Option<String>,
	featured_image: Option<&'a str>,
	author: Option<&'a str>,
	artist: Option<&'a str>,
	series_type: Option<&'a str>,
	series_status: Option<&'a str>,
	genres: Option<Vec<Genre<'a>>>,
	chapters: Option<Vec<IkenChapter<'a>>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IkenChapter<'a> {
	id: i32,
	slug: &'a str,
	number: f32,
	title: Option<&'a str>,
	created_by: Option<Author<'a>>,
	created_at: &'a str,
	// chapter_status: &'a str,
	is_locked: Option<bool>,
	is_time_locked: Option<bool>,
	pub content: Option<String>,
	pub images: Option<Vec<Image<'a>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Genre<'a> {
	// id: i32,
	name: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Author<'a> {
	name: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image<'a> {
	pub url: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct PostWithOnlyChapters<'a> {
	chapters: Option<Vec<IkenChapter<'a>>>,
}

impl Post<'_> {
	pub fn parse_basic_manga(&self, params: &Params) -> Manga {
		Manga {
			key: if params.use_slug_series_keys {
				self.slug.into()
			} else {
				self.id.to_string()
			},
			title: self.post_title.into(),
			cover: self.featured_image.map(|s| s.into()),
			..Default::default()
		}
	}

	pub fn parse_manga(&self, params: &Params) -> Manga {
		Manga {
			artists: self.artist.and_then(|s| {
				if !s.is_empty() {
					Some(vec![s.into()])
				} else {
					None
				}
			}),
			authors: self.author.and_then(|s| {
				if !s.is_empty() {
					Some(vec![s.into()])
				} else {
					None
				}
			}),
			description: self
				.post_content
				.as_ref()
				.and_then(|s| Html::parse_fragment(s).ok())
				.and_then(|html| {
					html.select_first("body")
						.expect("parsed fragment must have body")
						.text_with_newlines()
				})
				.map(|s| s.trim().into()),
			url: Some(format!("{}/series/{}", params.base_url, self.slug)),
			tags: self
				.genres
				.as_ref()
				.map(|genres| genres.iter().map(|genre| genre.name.into()).collect()),
			status: self
				.series_status
				.map(|s| match s {
					"ONGOING" | "COMING_SOON" => MangaStatus::Ongoing,
					"COMPLETED" | "ONE_SHOT" => MangaStatus::Completed,
					"CANCELLED" | "DROPPED" => MangaStatus::Cancelled,
					"HIATUS" => MangaStatus::Hiatus,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or(MangaStatus::Unknown),
			viewer: self
				.series_type
				.map(|s| match s {
					"MANGA" => Viewer::RightToLeft,
					"MANHUA" => Viewer::Webtoon,
					"MANHWA" => Viewer::Webtoon,
					_ => Viewer::Unknown,
				})
				.unwrap_or(Viewer::Unknown),
			..self.parse_basic_manga(params)
		}
	}

	pub fn chapters(&self, base_url: &str) -> Vec<Chapter> {
		self.chapters
			.as_ref()
			.map(|chapters| {
				chapters
					.iter()
					.map(|c| c.parse_chapter(base_url, self.slug))
					.collect()
			})
			.unwrap_or_default()
	}
}

impl PostWithOnlyChapters<'_> {
	pub fn chapters(&self, base_url: &str, slug: &str) -> Vec<Chapter> {
		self.chapters
			.as_ref()
			.map(|chapters| {
				chapters
					.iter()
					.map(|c| c.parse_chapter(base_url, slug))
					.collect()
			})
			.unwrap_or_default()
	}
}

impl IkenChapter<'_> {
	fn parse_chapter(&self, base_url: &str, manga_slug: &str) -> Chapter {
		Chapter {
			key: self.id.to_string(),
			title: self.title.and_then(|title| {
				if title.is_empty() {
					None
				} else {
					Some(title.into())
				}
			}),
			chapter_number: Some(self.number),
			volume_number: None,
			date_uploaded: chrono::DateTime::parse_from_rfc3339(self.created_at)
				.ok()
				.map(|d| d.timestamp()),
			scanlators: self
				.created_by
				.as_ref()
				.map(|author| vec![author.name.into()]),
			url: Some(format!("{base_url}/series/{manga_slug}/{}", self.slug)),
			locked: self.is_locked.or(self.is_time_locked).unwrap_or(false),
			..Default::default()
		}
	}
}

// impl From<Post<'_>> for Manga {
// 	fn from(value: Post<'_>) -> Self {
// 		value.parse_manga("BASE_URL")
// 	}
// }

// impl From<IkenChapter<'_>> for Chapter {
// 	fn from(value: IkenChapter<'_>) -> Self {
// 		value.parse_chapter("BASE_URL")
// 	}
// }
