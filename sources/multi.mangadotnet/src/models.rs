use crate::{BASE_URL, helpers::base_url_join};
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::{Deserialize, Deserializer, de, de::Error};

#[derive(Deserialize)]
pub struct PageContainer<T> {
	pub data: T,
}

#[derive(Deserialize)]
pub struct UserProfile {
	pub profile: Option<UserProfileData>,
}

#[derive(Deserialize)]
pub struct UserProfileData {
	pub id: Option<String>,
}

#[derive(Deserialize)]
pub struct BookmarkPage {
	pub data: BookmarkPageData,
}

#[derive(Deserialize)]
pub struct BookmarkPageData {
	pub total: i32,
	pub page: i32,
	pub per_page: i32,
	pub entries: Vec<BookmarkPageDataEntry>,
}

#[derive(Deserialize)]
pub struct BookmarkPageDataEntry {
	pub manga_id: i32,
	pub title: String,
	pub photo: Option<String>,
}

impl From<BookmarkPageDataEntry> for Manga {
	fn from(value: BookmarkPageDataEntry) -> Self {
		Self {
			key: value.manga_id.to_string(),
			title: value.title,
			cover: value.photo.map(|photo| base_url_join(&photo)),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewAllPage {
	pub data: ViewAllPageData,
}

#[derive(Deserialize)]
pub struct ViewAllPageData {
	pub manga_list: Vec<ShortMangaItem>,
	pub pagination: Pagination,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
	pub pagination: Option<Pagination>,
	pub results: Option<Vec<ShortMangaItem>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaDetailPage {
	pub manga_data: MangaDetailData,
}

#[derive(Deserialize)]
pub struct MangaDetailData {
	pub manga: MangaItem,
}

#[derive(Deserialize)]
pub struct ListingSectionData {
	pub items: Vec<ShortMangaItem>,
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
			Self::Single(s) => serde_json::from_str(&s).unwrap_or(vec![s]),
			Self::Multiple(v) => v,
		}
	}
}

#[derive(Deserialize)]
pub struct ShortMangaItem {
	pub id: i32,
	pub title: String,
	pub photo: Option<String>,
}

impl From<ShortMangaItem> for Manga {
	fn from(value: ShortMangaItem) -> Self {
		Self {
			key: value.id.to_string(),
			title: value.title,
			cover: value.photo.map(|photo| base_url_join(&photo)),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct MangaItem {
	pub id: i32,
	pub photo: Option<String>,
	pub title: String,
	pub artists: Option<StringOrVec>,
	pub authors: Option<StringOrVec>,
	pub status: String,
	#[serde(deserialize_with = "bool_from_any")]
	pub hiatus: bool,
	pub content_rating: Option<String>,
	#[serde(deserialize_with = "bool_from_any")]
	pub is_adult: bool,
	pub description: Option<String>,
	pub genres: Option<Vec<String>>,
	pub country_of_origin: Option<String>,
	#[serde(deserialize_with = "bool_from_any_optional")]
	pub is_longstrip: Option<bool>,
}

impl From<MangaItem> for Manga {
	fn from(value: MangaItem) -> Self {
		Self {
			key: value.id.to_string(),
			title: value.title,
			cover: value.photo.map(|photo| base_url_join(&photo)),
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
			content_rating: if value.is_adult {
				ContentRating::NSFW
			} else {
				value
					.content_rating
					.map_or(
						ContentRating::Unknown,
						|content_rating| match content_rating.as_str() {
							"safe" => ContentRating::Safe,
							"suggestive" => ContentRating::Suggestive,
							"erotica" => ContentRating::Suggestive,
							_ => ContentRating::Unknown,
						},
					)
			},
			viewer: value
				.country_of_origin
				.map_or(Viewer::Unknown, |coo| match coo.as_str() {
					"JP" => value.is_longstrip.map_or(Viewer::RightToLeft, |is_ls| {
						if is_ls {
							Viewer::Webtoon
						} else {
							Viewer::RightToLeft
						}
					}),
					"KR" => Viewer::Webtoon,
					"CN" => Viewer::Webtoon,
					_ => Viewer::Unknown,
				}),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct MangaGroup {
	pub id: i32,
	pub name: String,
}

#[derive(Deserialize)]
pub struct MangaChapter {
	pub id: i32,
	pub language: Option<String>,
	#[serde(deserialize_with = "f32_from_any_optional")]
	pub chapter_number: Option<f32>,
	#[serde(deserialize_with = "f32_from_any_optional")]
	pub volume_number: Option<f32>,
	pub chapter_title: Option<String>,
	pub groups: Option<Vec<MangaGroup>>,
	pub scanlator_name: Option<String>,
	pub date_added: String,
	pub uploader_username: Option<String>,
}

#[derive(Deserialize)]
pub struct MangaVolume {
	pub id: i32,
	pub language: String,
	pub volume_number: f32,
	pub cover_url: Option<String>,
	pub groups: Option<Vec<MangaGroup>>,
	pub scanlator_name: Option<String>,
	pub date_added: String,
	pub uploader_username: Option<String>,
}

impl MangaChapter {
	pub fn created_at(&self) -> Option<i64> {
		// Old upload is using old format and new uploads are using new format.
		// This should probably handle both.
		parse_date(&self.date_added, "yyyy-MM-dd HH:mm:ssZZZ").or(parse_date(
			&self.date_added,
			"yyyy-MM-dd HH:mm:ss.SSSSSSZZZ",
		))
	}
}

impl MangaVolume {
	pub fn created_at(&self) -> Option<i64> {
		// Old upload is using old format and new uploads are using new format.
		// This should probably handle both.
		parse_date(&self.date_added, "yyyy-MM-dd HH:mm:ssZZZ").or(parse_date(
			&self.date_added,
			"yyyy-MM-dd HH:mm:ss.SSSSSSZZZ",
		))
	}
}

impl From<MangaChapter> for Chapter {
	fn from(value: MangaChapter) -> Self {
		let date = value.created_at();
		Self {
			key: value.id.to_string(),
			title: value
				.chapter_title
				.filter(|title| !title.to_lowercase().starts_with("chapter")),
			chapter_number: value.chapter_number,
			volume_number: value.volume_number,
			date_uploaded: date,
			scanlators: value
				.groups
				.map(|g| g.into_iter().map(|group| group.name).collect::<Vec<_>>())
				.and_then(|s| if s.is_empty() { None } else { Some(s) })
				.or(value.scanlator_name.map(|name| vec![name])),
			url: value
				.uploader_username
				.map(|_| format!("{BASE_URL}/chapter/{}?source=user", value.id))
				.or(Some(format!("{BASE_URL}/chapter/{}", value.id))),
			language: value.language,
			..Default::default()
		}
	}
}

impl From<MangaVolume> for Chapter {
	fn from(value: MangaVolume) -> Self {
		Self {
			key: value.id.to_string(),
			title: None,
			chapter_number: None,
			volume_number: Some(value.volume_number),
			date_uploaded: value.created_at(),
			scanlators: value
				.groups
				.map(|g| g.into_iter().map(|group| group.name).collect::<Vec<_>>())
				.and_then(|s| if s.is_empty() { None } else { Some(s) })
				.or(value.scanlator_name.map(|name| vec![name])),
			url: value
				.uploader_username
				.map(|_| format!("{BASE_URL}/chapter/{}?source=user&mode=volume", value.id))
				.or(Some(format!("{BASE_URL}/chapter/{}?mode=volume", value.id))),
			language: Some(value.language),
			thumbnail: value
				.cover_url
				.map(|cover_url| format!("{BASE_URL}/{cover_url}")),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct MangaPage {
	pub chapter: MangaPageIdOnly,
	pub manga: MangaPageIdOnly,
	pub images: Vec<MangaPageImage>,
}

#[derive(Deserialize)]
pub struct MangaPageIdOnly {
	pub id: i32,
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
			formatter.write_str("a boolean that can be converted to bool")
		}

		fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
			Ok(v)
		}

		fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
			match v {
				0 => Ok(false),
				_ => Ok(true),
			}
		}

		fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
			match v {
				0 => Ok(false),
				_ => Ok(true),
			}
		}

		fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
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

		fn visit_unit<E>(self) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(false)
		}
	}

	deserializer.deserialize_any(BoolVisitor)
}

fn bool_from_any_optional<'de, D: Deserializer<'de>>(
	deserializer: D,
) -> Result<Option<bool>, D::Error> {
	struct BoolVisitor;

	impl<'de> de::Visitor<'de> for BoolVisitor {
		type Value = Option<bool>;

		fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
			formatter.write_str("a boolean that can be converted to bool")
		}

		fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
			Ok(Some(v))
		}

		fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
			match v {
				0 => Ok(Some(false)),
				_ => Ok(Some(true)),
			}
		}

		fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
			match v {
				0 => Ok(Some(false)),
				_ => Ok(Some(true)),
			}
		}

		fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
			match v.to_ascii_lowercase().as_str() {
				"true" => Ok(Some(true)),
				"false" => Ok(Some(false)),
				"1" => Ok(Some(true)),
				"0" => Ok(Some(false)),
				"yes" => Ok(Some(true)),
				"no" => Ok(Some(false)),
				_ => Err(E::custom(format!("invalid string for bool: {v}"))),
			}
		}

		fn visit_unit<E>(self) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(None)
		}
	}

	deserializer.deserialize_any(BoolVisitor)
}

fn f32_from_any_optional<'de, D: Deserializer<'de>>(
	deserializer: D,
) -> Result<Option<f32>, D::Error> {
	struct F32Visitor;

	impl<'de> de::Visitor<'de> for F32Visitor {
		type Value = Option<f32>;

		fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
			formatter.write_str("a number that can be converted to f32")
		}

		fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(Some(v as f32))
		}

		fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(Some(v as f32))
		}

		fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(Some(v as f32))
		}

		fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(Some(v as f32))
		}

		fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(Some(v))
		}

		fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(Some(v as f32))
		}

		fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
		where
			E: Error,
		{
			v.parse::<f32>().map(Some).map_err(Error::custom)
		}

		fn visit_unit<E>(self) -> Result<Self::Value, E>
		where
			E: Error,
		{
			Ok(None)
		}
	}

	deserializer.deserialize_any(F32Visitor)
}
