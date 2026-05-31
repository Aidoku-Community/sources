use crate::BASE_URL;
use aidoku::{
	Chapter, ContentRating, Link, Manga, MangaStatus, Viewer,
	alloc::string::ToString,
	alloc::vec,
	alloc::{String, Vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::{Deserialize, Deserializer, de};

#[derive(Deserialize)]
pub struct PageContainer<T> {
	pub data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomePageResponse {
	pub sections_data: HomeSection,
}

#[derive(Deserialize)]
pub struct HomeSection {
	pub sections: HomeSectionData,
}

#[derive(Deserialize)]
pub struct HomeSectionData {
	pub latest_updates: HomeSectionItem,
	pub recently_added: HomeSectionItem,
	pub most_tracked: HomeSectionItem,
	pub top_rated: HomeSectionItem,
}

#[derive(Deserialize)]
pub struct HomeSectionItem {
	pub items: Vec<MangaItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
	// pub all_genres: Vec<String>,
	pub pagination: Option<Pagination>,
	pub results: Option<Vec<MangaItem>>,
}

#[derive(Deserialize)]
pub struct Pagination {
	pub current_page: i32,
	pub total_pages: i32,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum StringOrVec {
	Single(String),
	Multiple(Vec<String>),
}

impl StringOrVec {
	fn into_vec(self) -> Vec<String> {
		match self {
			Self::Single(s) => {
				if let Ok(v) = serde_json::from_str(s.as_str()) {
					v
				} else {
					vec![s]
				}
			}
			Self::Multiple(v) => v,
		}
	}
}

#[derive(Deserialize)]
pub struct MangaItem {
	pub alt_titles: Option<StringOrVec>,
	pub artists: Option<StringOrVec>,
	pub authors: Option<StringOrVec>,
	// This is always null for Search endpoint.
	pub content_rating: Option<String>,
	pub country_of_origin: Option<String>,
	pub description: Option<String>,
	pub genres: Option<Vec<String>>,
	#[serde(deserialize_with = "bool_from_any")]
	pub hiatus: bool,
	pub id: i32,
	#[serde(deserialize_with = "bool_from_any")]
	pub is_blurworthy: bool,
	pub photo: Option<String>,
	pub status: String,
	pub title: String,
}

impl From<MangaItem> for Manga {
	fn from(value: MangaItem) -> Self {
		Self {
			key: value.id.to_string(),
			title: value.title,
			cover: if let Some(photo) = value.photo {
				photo.strip_prefix("/").map(|s| format!("{BASE_URL}/{s}"))
			} else {
				None
			},
			artists: value.artists.map(|a| a.into_vec()),
			authors: value.authors.map(|a| a.into_vec()),
			description: value.description,
			url: Some(format!("{BASE_URL}/manga/{}", value.id)),
			tags: value.genres,
			status: match value.status.as_str() {
				"Ongoing" => {
					if value.hiatus {
						MangaStatus::Hiatus
					} else {
						MangaStatus::Ongoing
					}
				}
				"Completed" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			},
			content_rating: if value.is_blurworthy {
				ContentRating::NSFW
			} else {
				if let Some(content_rating) = value.content_rating {
					match content_rating.as_str() {
						"safe" => ContentRating::Safe,
						"suggestive" => ContentRating::Suggestive,
						_ => ContentRating::Unknown,
					}
				} else {
					ContentRating::Unknown
				}
			},
			viewer: if let Some(coo) = value.country_of_origin {
				match coo.as_str() {
					"JP" => Viewer::RightToLeft,
					"KR" => Viewer::Webtoon,
					"CN" => Viewer::Webtoon,
					_ => Viewer::Unknown,
				}
			} else {
				Viewer::Unknown
			},
			..Default::default()
		}
	}
}

impl From<MangaItem> for Link {
	fn from(value: MangaItem) -> Self {
		let manga: Manga = value.into();
		manga.into()
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaDetailResponse {
	pub manga_data: MangaDetailData,
}

#[derive(Deserialize)]
pub struct MangaDetailData {
	pub manga: MangaItem,
}

#[derive(Deserialize)]
pub struct MangaChapter {
	pub id: i32,
	pub chapter_number: f32,
	pub volume_number: Option<f32>,
	pub chapter_title: Option<String>,
	pub language: Option<String>,
	pub group_id: Option<i32>,
	pub group_name: Option<String>,
	pub uploader_id: Option<i32>,
	pub uploader_username: Option<String>,
	pub date_added: String,
	pub source: String,
	pub scanlator_name: Option<String>,
}

impl MangaChapter {
	pub fn created_at(&self) -> Option<i64> {
		parse_date(&self.date_added, "yyyy-MM-dd HH:mm:ssZZZ")
	}
}

impl From<MangaChapter> for Chapter {
	fn from(value: MangaChapter) -> Self {
		let date = value.created_at();
		Self {
			key: value.id.to_string(),
			title: value.chapter_title,
			chapter_number: Some(value.chapter_number),
			volume_number: value.volume_number,
			date_uploaded: date,
			scanlators: value.scanlator_name.map(|name| vec![name]),
			url: if value.source == "user" {
				Some(format!("{BASE_URL}/chapter/{}?source=user", value.id))
			} else {
				Some(format!("{BASE_URL}/chapter/{}", value.id))
			},
			language: value.language,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct MangaPage {
	pub images: Vec<MangaPageImage>,
}

#[derive(Deserialize)]
pub struct MangaPageImage {
	pub url: String,
}

fn bool_from_any<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
	struct BoolVisitor;

	impl<'de> de::Visitor<'de> for BoolVisitor {
		type Value = bool;

		fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
			formatter.write_str("a boolean or 0/1")
		}

		fn visit_bool<E>(self, v: bool) -> Result<bool, E> {
			Ok(v)
		}

		fn visit_i64<E>(self, v: i64) -> Result<bool, E> {
			match v {
				0 => Ok(false),
				_ => Ok(true),
			}
		}

		fn visit_u64<E>(self, v: u64) -> Result<bool, E> {
			match v {
				0 => Ok(false),
				_ => Ok(true),
			}
		}

		fn visit_str<E: de::Error>(self, v: &str) -> Result<bool, E> {
			match v.to_ascii_lowercase().as_str() {
				"true" => Ok(true),
				"false" => Ok(false),
				"1" => Ok(true),
				"0" => Ok(false),
				"yes" => Ok(true),
				"no" => Ok(false),
				_ => Err(E::custom(format!("invalid string for bool: {v}"))),
			}
		}

		fn visit_none<E>(self) -> Result<bool, E> {
			Ok(false)
		}
	}

	deserializer.deserialize_any(BoolVisitor)
}
