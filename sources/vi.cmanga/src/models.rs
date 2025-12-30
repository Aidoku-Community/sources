use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, MangaWithChapter, Viewer,
	alloc::{
		borrow::ToOwned,
		format,
		string::{String, ToString},
		vec::Vec,
	},
	imports::std::get_utc_offset,
	serde::Deserializer,
};
use serde::{Deserialize, Serialize};

use crate::BASE_URL;

fn deserialize_str_from_any<'de, D>(deserializer: D) -> Result<String, D::Error>
where
	D: Deserializer<'de>,
{
	let v = serde_json::Value::deserialize(deserializer)?;

	match v {
		serde_json::Value::String(s) => Ok(s),
		serde_json::Value::Number(n) => Ok(n.to_string()),
		serde_json::Value::Bool(b) => Ok(b.to_string()),
		serde_json::Value::Null => Ok(String::new()),
		_ => Err(serde::de::Error::custom("expected string or number")),
	}
}

fn deserialize_f32_from_any<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
	D: Deserializer<'de>,
{
	let v = serde_json::Value::deserialize(deserializer)?;

	match v {
		serde_json::Value::String(s) => Ok(s.parse::<f32>().ok().unwrap_or(-1.0)),
		serde_json::Value::Number(n) => Ok(n.as_f64().map(|v| v as f32).unwrap_or(-1.0)),
		serde_json::Value::Bool(b) => Ok(if b { 1.0 } else { 0.0 }),
		serde_json::Value::Null => Ok(0.0),
		_ => Err(serde::de::Error::custom("expected string or number")),
	}
}

fn deserialize_i64_from_any<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
	D: Deserializer<'de>,
{
	let v = serde_json::Value::deserialize(deserializer)?;

	match v {
		serde_json::Value::String(s) => Ok(s.parse::<i64>().ok().unwrap_or(-1)),
		serde_json::Value::Number(n) => Ok(n.as_i64().unwrap_or(0)),
		serde_json::Value::Bool(b) => Ok(if b { 1 } else { 0 }),
		serde_json::Value::Null => Ok(0),
		_ => Err(serde::de::Error::custom("expected string or number")),
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaChapter {
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub id: String,

	#[serde(deserialize_with = "deserialize_f32_from_any")]
	pub last: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MangaInfo {
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub id: String,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub url: String,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub name: String,
	pub tags: Vec<String>,
	pub avatar: String,
	pub detail: Option<String>,
	// pub hidden: Option<u8>,
	pub source: Option<String>,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub status: String,
	pub chapter: MangaChapter,
	// pub block_ads: Option<String>,
	pub url_other: Option<Vec<String>>,
	pub name_other: Option<Vec<String>>,
	pub author: Option<Vec<String>>,
}

fn deserialize_manga_info<'de, D>(deserializer: D) -> Result<MangaInfo, D::Error>
where
	D: Deserializer<'de>,
{
	let s = String::deserialize(deserializer)?;
	serde_json::from_str(&s).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaItem {
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub id_album: String,
	#[serde(deserialize_with = "deserialize_manga_info")]
	pub info: MangaInfo,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub last_update: String,
}

fn capitalize(value: &str) -> String {
	value
		.split_whitespace()
		.map(|w| {
			let mut chars = w.chars();
			match chars.next() {
				Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
				None => String::new(),
			}
		})
		.collect::<Vec<_>>()
		.join(" ")
}

fn get_viewer(categories: &[String]) -> (ContentRating, Viewer) {
	let mut nsfw = ContentRating::Unknown;
	let mut viewer = Viewer::RightToLeft;

	for category in categories {
		match category.to_lowercase().as_str() {
			"smut" | "mature" | "18+" | "adult" => nsfw = ContentRating::NSFW,
			"ecchi" | "16+" => {
				if nsfw != ContentRating::NSFW {
					nsfw = ContentRating::Suggestive
				}
			}
			"webtoon" | "manhwa" | "manhua" => viewer = Viewer::Webtoon,
			_ => continue,
		}
	}

	(nsfw, viewer)
}
impl From<MangaInfo> for Manga {
	fn from(value: MangaInfo) -> Self {
		let tags = value
			.tags
			.into_iter()
			.map(|v| capitalize(&v))
			.collect::<Vec<_>>();
		let (content_rating, viewer) = get_viewer(&tags);
		Self {
			key: value.id.to_string(),
			title: capitalize(&value.name),
			cover: format!("{}/assets/tmp/album/{}", BASE_URL, value.avatar).into(),
			artists: value.source.map(|v| [v].to_vec()),
			authors: value
				.author
				.and_then(|v| if v.is_empty() { None } else { Some(v) }),
			description: value.detail,
			url: Some(format!("{}/album/{}-{}", BASE_URL, value.url, value.id)),
			tags: Some(tags),
			status: match value.status.to_lowercase().as_str() {
				"doing" => MangaStatus::Ongoing,
				"drop" => MangaStatus::Cancelled,
				"done" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			},
			content_rating,
			viewer,
			..Default::default()
		}
	}
}
impl From<MangaItem> for Manga {
	fn from(value: MangaItem) -> Self {
		value.info.into()
	}
}
impl From<MangaItem> for MangaWithChapter {
	fn from(value: MangaItem) -> Self {
		let id = value.info.chapter.id.clone();
		let last = value.info.chapter.last;

		Self {
			manga: value.into(),
			chapter: Chapter {
				key: id,
				chapter_number: Some(last),
				..Default::default()
			},
		}
	}
}

#[derive(Deserialize)]
pub struct MangaResults {
	pub data: Vec<MangaItem>,

	#[serde(deserialize_with = "deserialize_i64_from_any")]
	pub total: i64,
}

#[derive(Deserialize)]
pub struct MangaResult {
	#[serde(deserialize_with = "deserialize_manga_info")]
	pub info: MangaInfo,
}

/// ======================================= chapter ===================================================
/// Custom deserializer: parse JSON string → ChapterInfo
fn deserialize_chapter_info<'de, D>(deserializer: D) -> Result<ChapterInfo, D::Error>
where
	D: Deserializer<'de>,
{
	let s = String::deserialize(deserializer)?;
	serde_json::from_str(&s).map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
pub struct MChapter {
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub id_chapter: String,

	#[serde(deserialize_with = "deserialize_chapter_info")]
	pub info: ChapterInfo,
}
use chrono::{FixedOffset, NaiveDateTime, TimeZone};
fn parse_datetime_to_timestamp(s: &str) -> Option<i64> {
	// Format "YYYY-MM-DD HH:MM:SS"
	let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
	let offset = FixedOffset::east_opt(get_utc_offset() as i32)?;

	let dt = offset.from_local_datetime(&naive).single()?;
	Some(dt.timestamp())
}
impl From<MChapter> for Chapter {
	fn from(value: MChapter) -> Self {
		Self {
			key: value.info.id,
			title: if value.info.name.is_empty() {
				None
			} else {
				Some(value.info.name)
			},
			chapter_number: value.info.num.parse::<f32>().ok(),
			date_uploaded: parse_datetime_to_timestamp(&value.info.last_update),
			url: None,
			language: Some("vi".to_owned()),
			locked: value.info.lock.is_some(),
			..Default::default()
		}
	}
}

#[derive(Debug, Deserialize)]
pub struct ChapterLock {}
#[derive(Debug, Deserialize)]
pub struct ChapterInfo {
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub name: String,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub source: String,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub num: String,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub last_update: String,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub album: String,
	#[serde(deserialize_with = "deserialize_str_from_any")]
	pub id: String,
	pub upload_num: u32,
	pub modified: Option<String>,
	pub lock: Option<ChapterLock>,
}

#[derive(Deserialize)]
pub struct ChapterImages {
	pub image: Vec<String>,
}
