use crate::{BASE_URL, helpers, settings};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Page, PageContent, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
};
use serde::{Deserialize, Deserializer, de};

#[derive(Deserialize)]
pub struct SearchResponse {
	#[serde(deserialize_with = "manga_items_or_vec")]
	pub result: MangaItems,
}

fn manga_items_or_vec<'de, D>(deserializer: D) -> Result<MangaItems, D::Error>
where
	D: Deserializer<'de>,
{
	let value = serde_json::Value::deserialize(deserializer)?;

	if let Some(items) = value.get("items") {
		let items: Vec<ComixManga> =
			serde_json::from_value(items.clone()).map_err(serde::de::Error::custom)?;
		let meta = value
			.get("meta")
			.map(|m| serde_json::from_value(m.clone()).map_err(serde::de::Error::custom))
			.transpose()?;
		Ok(MangaItems { items, meta })
	} else if value.is_array() {
		let items: Vec<ComixManga> =
			serde_json::from_value(value).map_err(serde::de::Error::custom)?;
		Ok(MangaItems { items, meta: None })
	} else {
		Err(serde::de::Error::custom(
			"Invalid MangaItems or Vec<ComixManga>",
		))
	}
}

impl From<SearchResponse> for MangaPageResult {
	fn from(value: SearchResponse) -> Self {
		value.result.into()
	}
}

#[derive(Deserialize)]
pub struct SingleMangaResponse {
	pub result: ComixManga,
}

#[derive(Deserialize)]
pub struct ChapterDetailsResponse {
	pub result: ChapterItems,
}

#[derive(Deserialize)]
pub struct ChapterResponse {
	pub result: Option<ComixChapterWithPages>,
}

#[derive(Deserialize)]
pub struct TermResponse {
	pub result: TermItems,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
	pub page: i32,
	pub last_page: i32,
}

#[derive(Deserialize)]
pub struct MangaItems {
	pub items: Vec<ComixManga>,
	pub meta: Option<Pagination>,
}

impl MangaItems {
	pub fn into_filtered(self, content_types: &[String], hidden_terms: &[i32]) -> MangaPageResult {
		MangaPageResult {
			entries: self
				.items
				.into_iter()
				.filter(|m| !m.is_hidden(content_types, hidden_terms))
				.map(Into::into)
				.collect(),
			has_next_page: self.meta.map(|p| p.page < p.last_page).unwrap_or_default(),
		}
	}
}

impl From<MangaItems> for MangaPageResult {
	fn from(value: MangaItems) -> Self {
		MangaPageResult {
			entries: value.items.into_iter().map(Into::into).collect(),
			has_next_page: value.meta.map(|p| p.page < p.last_page).unwrap_or_default(),
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterItems {
	pub items: Vec<ComixChapter>,
	pub meta: Pagination,
}

#[derive(Deserialize)]
pub struct TermItems {
	pub items: Vec<Term>,
	// pub pagination: Pagination,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComixManga {
	pub hid: String,
	pub title: String,
	pub synopsis: Option<String>,
	pub r#type: String,
	pub poster: Poster,
	pub status: String,
	pub content_rating: ComixContentRating,
	pub authors: Option<Vec<Term>>,
	pub artists: Option<Vec<Term>>,
	pub genres: Option<Vec<Term>>,
	pub tags: Option<Vec<Term>>,
	pub latest_chapter: Option<f32>,
	// pub has_chapters: bool,
	// pub chapter_updated_at_formatted: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ComixContentRating {
	Safe,
	Suggestive,
	Erotica,
	Pornographic,
	#[default]
	#[serde(other)]
	Unknown,
}

impl From<ComixContentRating> for ContentRating {
	fn from(value: ComixContentRating) -> Self {
		match value {
			ComixContentRating::Safe => ContentRating::Safe,
			ComixContentRating::Suggestive => ContentRating::Suggestive,
			ComixContentRating::Erotica => ContentRating::NSFW,
			ComixContentRating::Pornographic => ContentRating::NSFW,
			ComixContentRating::Unknown => ContentRating::Unknown,
		}
	}
}

impl ComixManga {
	pub fn is_hidden(&self, hidden_types: &[String], hidden_terms: &[i32]) -> bool {
		if hidden_types.contains(&self.r#type) {
			return true;
		}

		if !hidden_terms.is_empty() {
			let tag_match = self
				.tags
				.as_ref()
				.map(|tags| tags.iter().any(|term| hidden_terms.contains(&term.id)))
				.unwrap_or(false);
			if tag_match {
				true
			} else {
				self.genres
					.as_ref()
					.map(|genres| genres.iter().any(|term| hidden_terms.contains(&term.id)))
					.unwrap_or(false)
			}
		} else {
			false
		}
	}
}

impl From<ComixManga> for Manga {
	fn from(value: ComixManga) -> Self {
		let url = format!("{BASE_URL}/title/{}", value.hid);
		Self {
			key: value.hid,
			title: value.title,
			cover: match settings::image_quality().as_str() {
				"medium" => Some(value.poster.medium),
				"large" => Some(value.poster.large),
				_ => Some(value.poster.medium),
			},
			artists: value
				.artists
				.map(|v| v.into_iter().map(|t| t.title).collect()),
			authors: value
				.authors
				.map(|v| v.into_iter().map(|t| t.title).collect()),
			description: value.synopsis,
			url: Some(url),
			tags: {
				let mut tags = Vec::new();
				if let Some(genres) = value.genres {
					tags.extend(genres.into_iter().map(|t| t.title));
				}
				if let Some(tags_vec) = value.tags {
					tags.extend(tags_vec.into_iter().map(|t| t.title));
				}
				if tags.is_empty() { None } else { Some(tags) }
			},
			status: match value.status.as_str() {
				"releasing" => MangaStatus::Ongoing,
				"on_hiatus" => MangaStatus::Hiatus,
				"finished" => MangaStatus::Completed,
				"discontinued" => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			content_rating: value.content_rating.into(),
			viewer: match value.r#type.as_str() {
				"manhwa" => Viewer::Webtoon,
				"manhua" => Viewer::Webtoon,
				"manga" => Viewer::RightToLeft,
				_ => Viewer::Unknown,
			},
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComixChapter {
	pub id: i32,
	pub number: f32,
	pub name: String,
	pub votes: i32,
	pub created_at_formatted: String,
	pub group: Option<ScanlationGroup>,
	#[serde(deserialize_with = "bool_from_any")]
	pub is_official: bool,
}

impl ComixChapter {
	pub fn created_at(&self) -> i64 {
		helpers::parse_relative_date_string(&self.created_at_formatted)
	}

	pub fn into_chapter(self, manga_id: &str) -> Chapter {
		let created_at = self.created_at();
		Chapter {
			key: self.id.to_string(),
			title: (!self.name.is_empty()).then_some(self.name),
			chapter_number: Some(self.number),
			date_uploaded: Some(created_at),
			scanlators: if let Some(group) = self.group {
				Some(vec![group.name])
			} else if self.is_official {
				Some(vec!["Official".into()])
			} else {
				None
			},
			url: Some(format!("{BASE_URL}/title/{manga_id}/{}", self.id)),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ComixChapterWithPages {
	pub pages: Vec<ComixPage>,
}

#[derive(Deserialize)]
pub struct Poster {
	pub medium: String,
	pub large: String,
}

#[derive(Deserialize)]
pub struct Term {
	pub id: i32,
	pub title: String,
}

#[derive(Deserialize)]
pub struct ScanlationGroup {
	pub id: i32,
	pub name: String,
}

#[derive(Deserialize)]
pub struct ComixPage {
	pub url: String,
}

impl From<ComixPage> for Page {
	fn from(value: ComixPage) -> Self {
		Page {
			content: PageContent::url(value.url),
			..Default::default()
		}
	}
}

// deserialize a bool from a json bool, number, or string
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

		fn visit_u64<E>(self, v: u64) -> Result<bool, E> {
			match v {
				0 => Ok(false),
				_ => Ok(true),
			}
		}

		fn visit_i64<E>(self, v: i64) -> Result<bool, E> {
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
				_ => Err(E::custom(format!("invalid string for bool: {v}"))),
			}
		}

		fn visit_none<E>(self) -> Result<bool, E> {
			Ok(false)
		}
	}

	deserializer.deserialize_any(BoolVisitor)
}
