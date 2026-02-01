use crate::{
	COVER_URL,
	settings::{self, TitlePreference},
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec},
	prelude::format,
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Default, Deserialize, Serialize, Debug, Clone)]
pub struct TokenResponse {
	pub access_token: Option<String>,
	pub refresh_token: Option<String>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexResponse<T> {
	// pub result: &'a str,
	// pub response: &'a str,
	pub data: T,
	// pub offset: Option<i32>,
	pub total: Option<i32>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexStatusResponse {
	// pub result: &'a str,
	pub statuses: Map<String, Value>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexRelationship<'a> {
	pub id: &'a str,
	pub r#type: &'a str,
	pub attributes: Option<Map<String, Value>>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", default)]
pub struct DexLocalizedString {
	pub en: Option<String>,
	pub ja: Option<String>,
	pub ja_ro: Option<String>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexTag {
	// pub id: String,
	pub attributes: DexTagAttributes,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexTagAttributes {
	pub name: DexLocalizedString,
}

// generic data result without attribuets
#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexData<'a> {
	pub id: &'a str,
	// pub relationships: Vec<DexRelationship<'a>>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexCustomList<'a> {
	pub id: &'a str,
	pub attributes: DexCustomListAttributes,
	pub relationships: Vec<DexRelationship<'a>>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexCoverArt {
	// pub id: &'a str,
	pub attributes: DexCoverArtAttributes,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexManga<'a> {
	pub id: &'a str,
	pub attributes: DexMangaAttributes,
	pub relationships: Vec<DexRelationship<'a>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum DexContentRating {
	#[default]
	Safe,
	Suggestive,
	Erotica,
	Pornographic,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum DexStatus {
	Completed,
	#[default]
	Ongoing,
	Cancelled,
	Hiatus,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexCustomListAttributes {
	pub name: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct DexCoverArtAttributes {
	pub volume: Option<String>,
	pub file_name: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct DexMangaAttributes {
	pub title: Value,
	pub alt_titles: Vec<Value>,
	pub description: DexLocalizedString,
	pub original_language: String,
	pub content_rating: DexContentRating,
	pub tags: Vec<DexTag>,
	pub status: DexStatus,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DexChapter<'a> {
	pub id: &'a str,
	pub attributes: DexChapterAttributes<'a>,
	pub relationships: Vec<DexRelationship<'a>>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct DexChapterAttributes<'a> {
	pub title: Option<String>,
	pub volume: Option<&'a str>,
	pub chapter: Option<&'a str>,
	pub external_url: Option<Value>,
	pub is_unavailable: bool,
	pub translated_language: &'a str,
	pub publish_at: &'a str,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct DexAtHomeResponse<'a> {
	pub base_url: String,
	#[serde(borrow)]
	pub chapter: DexAtHomeChapter<'a>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct DexAtHomeChapter<'a> {
	pub hash: &'a str,
	pub data: Option<Vec<&'a str>>,
	pub data_saver: Option<Vec<&'a str>>,
}

impl DexLocalizedString {
	pub fn get(&self) -> Option<String> {
		if let Some(en) = &self.en {
			Some(en.clone())
		} else if let Some(ja_ro) = &self.ja_ro {
			Some(ja_ro.clone())
		} else {
			self.ja.clone()
		}
	}
}

impl DexManga<'_> {
	pub fn into_basic_manga(self) -> Manga {
		Manga {
			key: String::from(self.id),
			title: self.title().unwrap_or_default(),
			cover: self.cover(),
			..Default::default()
		}
	}

	pub fn title(&self) -> Option<String> {
		let preference = settings::get_title_preference();
		let primary_title = self
			.attributes
			.title
			.as_object()
			.and_then(|o| o.values().next())
			.and_then(|v| v.as_str().map(Into::into));
		if preference == TitlePreference::Primary {
			return primary_title;
		}
		let alt_titles = self.attributes.alt_titles.clone().into_iter().rev().fold(
			Map::new(),
			|mut acc, mut v| {
				if let Some(map) = v.as_object_mut() {
					acc.extend(core::mem::take(map));
				}
				acc
			},
		);
		match preference {
			TitlePreference::SelectedLanguage => {
				if let Ok(languages) = settings::get_languages() {
					for lang in languages {
						let title = alt_titles.get(&lang);
						if let Some(title) = title {
							return title.as_str().map(Into::into);
						}
					}
				}
				primary_title
			}
			TitlePreference::English => alt_titles
				.get("en")
				.and_then(|v| v.as_str().map(Into::into))
				.or(primary_title),
			TitlePreference::Romaji => alt_titles
				.get("ja-ro")
				.or_else(|| alt_titles.get("ko-ro"))
				.and_then(|v| v.as_str().map(Into::into))
				.or(primary_title),
			TitlePreference::Japanese => alt_titles
				.get("ja")
				.and_then(|v| v.as_str().map(Into::into))
				.or(primary_title),
			_ => unreachable!(),
		}
	}

	pub fn description(&self) -> Option<String> {
		self.attributes.description.get()
	}

	pub fn cover(&self) -> Option<String> {
		self.relationships.iter().find_map(|r| {
			if r.r#type == "cover_art" {
				Some(format!(
					"{COVER_URL}/covers/{}/{}{}",
					self.id,
					r.attributes
						.clone()
						.and_then(|v| v.get("fileName").map(|v| v.as_str().map(String::from)))
						.flatten()
						.unwrap_or_default(),
					settings::get_cover_quality()
				))
			} else {
				None
			}
		})
	}

	pub fn authors(&self) -> Vec<String> {
		self.relationships
			.iter()
			.filter(|r| r.r#type == "author")
			.filter_map(|r| {
				r.attributes
					.as_ref()
					.map(|a| a.get("name").map(|v| v.as_str().map(String::from)))
			})
			.flatten()
			.flatten()
			.collect()
	}

	pub fn artists(&self) -> Vec<String> {
		self.relationships
			.iter()
			.filter(|r| r.r#type == "artist")
			.filter_map(|r| {
				r.attributes
					.as_ref()
					.map(|a| a.get("name").map(|v| v.as_str().map(String::from)))
			})
			.flatten()
			.flatten()
			.collect()
	}

	pub fn url(&self) -> String {
		format!("https://mangadex.org/title/{}", self.id)
	}

	pub fn tags(&self) -> Vec<String> {
		self.attributes
			.tags
			.iter()
			.filter_map(|t| t.attributes.name.get())
			.collect()
	}

	pub fn status(&self) -> MangaStatus {
		match self.attributes.status {
			DexStatus::Ongoing => MangaStatus::Ongoing,
			DexStatus::Completed => MangaStatus::Completed,
			DexStatus::Hiatus => MangaStatus::Hiatus,
			DexStatus::Cancelled => MangaStatus::Cancelled,
		}
	}

	pub fn content_rating(&self) -> ContentRating {
		match self.attributes.content_rating {
			DexContentRating::Safe => ContentRating::Safe,
			DexContentRating::Suggestive => ContentRating::Suggestive,
			DexContentRating::Erotica | DexContentRating::Pornographic => ContentRating::NSFW,
		}
	}
}

impl From<DexManga<'_>> for Manga {
	fn from(val: DexManga<'_>) -> Self {
		let tags = val.tags();
		let viewer = if tags.iter().any(|t| t == "Long Strip") {
			Viewer::Webtoon
		} else {
			match val.attributes.original_language.as_str() {
				"ja" => Viewer::RightToLeft,
				"zh" | "ko" => Viewer::Webtoon,
				_ => Viewer::RightToLeft,
			}
		};
		Manga {
			key: String::from(val.id),
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

impl<'a> DexChapter<'a> {
	pub fn has_external_url(&self) -> bool {
		self.attributes.external_url.is_some()
	}

	pub fn url(&self) -> String {
		format!("https://mangadex.org/chapter/{}", self.id)
	}

	pub fn manga_id(&self) -> Option<&'a str> {
		self.relationships.iter().find_map(|r| {
			if r.r#type == "manga" {
				Some(r.id)
			} else {
				None
			}
		})
	}

	pub fn scanlators(&self) -> Vec<String> {
		let scanlation_groups: Vec<String> = self
			.relationships
			.iter()
			.filter_map(|r| {
				if r.r#type == "scanlation_group" {
					r.attributes
						.clone()
						.and_then(|v| v.get("name").map(|v| v.as_str().map(String::from)))
						.flatten()
				} else {
					None
				}
			})
			.collect();
		if scanlation_groups.is_empty() {
			self.relationships
				.iter()
				.filter_map(|r| {
					if r.r#type == "user" {
						r.attributes
							.clone()
							.and_then(|v| v.get("username").map(|v| v.as_str().map(String::from)))
							.flatten()
					} else {
						None
					}
				})
				.collect()
		} else {
			scanlation_groups
		}
	}

	// pub fn manga(&self) -> Option<DexManga> {
	// 	self.relationships.iter().find_map(|r| {
	// 		if r.r#type == "manga" {
	// 			let attributes: Option<DexMangaAttributes> = r
	// 				.attributes
	// 				.clone()
	// 				.map(serde_json::Value::Object)
	// 				.map(serde_json::from_value)
	// 				.map(|v| v.ok())
	// 				.flatten();
	// 			if let Some(attributes) = attributes {
	// 				return Some(DexManga {
	// 					id: r.id,
	// 					attributes,
	// 					relationships: Vec::new(),
	// 				});
	// 			}
	// 		}
	// 		None
	// 	})
	// }
}

impl From<DexChapter<'_>> for Chapter {
	fn from(val: DexChapter<'_>) -> Self {
		let chapter_number = val.attributes.chapter.and_then(|v| v.parse::<f32>().ok());
		let volume_number = val.attributes.volume.and_then(|v| v.parse::<f32>().ok());

		// As per MangaDex upload guidelines, if the volume and chapter are both null or
		// for serialized entries, the volume is 0 and chapter is null, it's a oneshot.
		// They should have a title of "Oneshot" but some don't, so we'll add it if it's missing.
		let title = if (volume_number.is_none() || volume_number == Some(0.0))
			&& chapter_number.is_none()
			&& val.attributes.title.as_ref().is_none_or(|t| t.is_empty())
		{
			Some(String::from("Oneshot"))
		} else {
			val.attributes.title.clone()
		};

		Chapter {
			key: String::from(val.id),
			title,
			chapter_number,
			volume_number,
			date_uploaded: DateTime::parse_from_rfc3339(val.attributes.publish_at)
				.ok()
				.map(|d| d.timestamp()),
			scanlators: Some(val.scanlators()),
			url: Some(val.url()),
			language: Some(String::from(val.attributes.translated_language)),
			locked: val.attributes.is_unavailable,
			..Default::default()
		}
	}
}
