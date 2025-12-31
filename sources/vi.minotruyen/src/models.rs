use crate::BASE_URL;
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, MangaWithChapter, Viewer,
	alloc::{
		borrow::ToOwned,
		string::{String, ToString},
		vec::Vec,
	},
	imports::std::get_utc_offset,
	prelude::*,
};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
pub struct FeaturedRoot {
	pub success: bool,
	pub data: FeaturedBooksData,
}

#[derive(Debug, Deserialize)]
pub struct SideHomeRoot {
	pub success: bool,
	#[serde(rename = "topBooksView")]
	pub top_books_view: Vec<BookItem>,
}

#[derive(Debug, Deserialize)]
pub struct FeaturedBooksData {
	pub books: Vec<BookItem>,
	#[serde(rename = "countBook")]
	pub count_books: Option<i32>,
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<Option<Vec<Tag>>, D::Error>
where
	D: Deserializer<'de>,
{
	use serde_json::Value;

	let v = Option::<Vec<Value>>::deserialize(deserializer)?;

	let Some(items) = v else {
		return Ok(None);
	};

	let mut out = Vec::with_capacity(items.len());

	for item in items {
		if let Ok(tag) = Tag::deserialize(item.clone()) {
			out.push(tag);
			continue;
		}

		if let Ok(wrapper) = TagWrapper::deserialize(item.clone()) {
			out.push(wrapper.tag);
			continue;
		}

		return Err(serde::de::Error::custom("Invalid tag format"));
	}

	Ok(Some(out))
}

#[derive(Debug, Deserialize)]
pub struct BookItem {
	#[serde(rename = "type")]
	pub r#type: Option<String>,
	pub slug: String,
	pub title: String,
	#[serde(rename = "bookId")]
	pub book_id: i64,
	pub status: i64,
	pub category: String,
	pub thumbnail: Option<String>,
	#[serde(rename = "createdAt")]
	pub created_at: Option<String>,
	#[serde(rename = "updatedAt")]
	pub updated_at: Option<String>,
	pub description: Option<String>,
	#[serde(rename = "anotherName")]
	pub another_name: Option<String>,

	pub chapters: Option<Vec<VChapter>>,

	#[serde(deserialize_with = "deserialize_tags")]
	pub tags: Option<Vec<Tag>>,
	pub authors: Option<Vec<Author>>,
	pub covers: Vec<Cover>,
}
impl From<BookItem> for Manga {
	fn from(value: BookItem) -> Self {
		let tags = value
			.tags
			.unwrap_or_default()
			.into_iter()
			.map(|t| t.name)
			.collect::<Vec<_>>();
		let (content_rating, viewer) = get_viewer(&tags, &value.category);
		Self {
			key: format!("{}/{}-{}", value.category, value.slug, value.book_id),
			title: value.title,
			cover: value.covers.first().map(|c| c.url.to_owned()),
			artists: value.r#type.map(|v| aidoku::alloc::vec![capitalize(&v)]),
			authors: value
				.authors
				.map(|v| v.into_iter().map(|v| v.name).collect::<Vec<_>>()),
			description: value.description,
			url: Some(format!(
				"{BASE_URL}/{}/books/{}-{}",
				value.category, value.slug, value.book_id
			)),
			tags: if tags.is_empty() { None } else { Some(tags) },
			status: if value.status == 1 {
				MangaStatus::Ongoing
			} else {
				MangaStatus::Completed
			},
			content_rating,
			viewer,
			chapters: value
				.chapters
				.map(|v| v.into_iter().map(|c| c.into()).collect::<Vec<_>>()),
			..Default::default()
		}
	}
}
impl From<BookItem> for MangaWithChapter {
	fn from(value: BookItem) -> Self {
		let chapter = value
			.chapters
			.as_ref()
			.and_then(|v| v.first().map(|v| v.clone()))
			.unwrap_or_default()
			.into();
		Self {
			manga: value.into(),
			chapter,
		}
	}
}

#[derive(Debug, Deserialize)]
pub struct Cover {
	pub url: String,
	pub index: i32,
	pub width: i32,
	pub height: i32,

	#[serde(rename = "dominantColor")]
	pub dominant_color: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct VChapter {
	#[serde(rename = "createdAt")]
	pub created_at: Option<String>,

	#[serde(rename = "chapterNumber")]
	pub chapter_number: f32,
	pub num: Option<String>,
}
impl From<VChapter> for Chapter {
	fn from(value: VChapter) -> Self {
		Self {
			key: format!("chapter-{}-{}", value.chapter_number, value.chapter_number),
			chapter_number: Some(value.chapter_number),
			title: value.num.map(|v| format!("Chap {v}")),
			date_uploaded: value
				.created_at
				.and_then(|v| parse_datetime_to_timestamp(&v)),
			..Default::default()
		}
	}
}

#[derive(Debug, Deserialize)]
pub struct Tag {
	pub name: String,
	#[serde(alias = "tagId")]
	pub slug: String,
}
#[derive(Debug, Deserialize)]
pub struct TagWrapper {
	pub tag: Tag,
}

#[derive(Debug, Deserialize)]
pub struct Author {
	pub name: String,
}

pub fn capitalize(s: &str) -> String {
	let mut chars = s.chars();

	match chars.next() {
		None => String::new(),
		Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
	}
}

fn get_viewer(categories: &[String], category: &str) -> (ContentRating, Viewer) {
	let mut nsfw = ContentRating::Unknown;
	let mut viewer = if category == "manga" {
		Viewer::RightToLeft
	} else {
		Viewer::LeftToRight
	};

	for category in categories {
		match category.to_lowercase().as_str() {
			"smut" | "mature" | "18+" | "adult" => nsfw = ContentRating::NSFW,
			"ecchi" | "16+" => {
				if nsfw != ContentRating::NSFW {
					nsfw = ContentRating::Suggestive
				}
			}
			"webtoon" | "manhwa" | "manhua" => viewer = Viewer::Webtoon,
			"manga" => viewer = Viewer::RightToLeft,
			_ => continue,
		}
	}

	(nsfw, viewer)
}

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
fn parse_datetime_to_timestamp(s: &str) -> Option<i64> {
	// Format "YYYY-MM-DD HH:MM:SS"
	let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
	let offset = FixedOffset::east_opt(get_utc_offset() as i32)?;

	let dt = offset.from_local_datetime(&naive).single()?;
	Some(dt.timestamp())
}

#[derive(Deserialize)]
pub struct FlightRoot<T> {
	pub children: T,
}
#[derive(Deserialize)]
pub struct FlightChild<T>(pub String, pub String, pub Option<String>, pub T);
#[derive(Deserialize)]
#[serde(untagged)]
pub enum FlightAny<T> {
	Str(String),
	Arr(FlightChild<T>),
}

#[derive(Debug, Deserialize)]
pub struct FlightNode {
	pub book: BookItem,
	pub content: String,
	#[serde(rename = "currentDate")]
	pub current_date: String,
}

pub fn extract_next_object(input: &str, skip: Option<usize>) -> Option<String> {
	let input = input.replace("\\\"", "\"");
	let bytes = input.as_bytes();

	let mut start = None;
	let mut brace_count = 0;

	let mut skip = skip.unwrap_or_default();
	for (i, &b) in bytes.iter().enumerate() {
		if b == b'{' {
			if i < skip {
				skip -= 1;
				continue;
			}
			if start.is_none() {
				start = Some(i);
			}
			brace_count += 1;
		} else if b == b'}' {
			if brace_count > 0 {
				brace_count -= 1;
				if brace_count == 0 {
					let s = start.unwrap();
					let json_str = &input[s..=i];
					return Some(json_str.to_string());
				}
			}
		}
	}

	None
}

#[derive(serde::Deserialize)]
pub struct VChapterF {
	pub book_id: u64,
	pub num: String,
	pub chapter_number: f32,
	pub title: Option<String>,
	pub created_at: Option<DateTime<Utc>>,
	pub updated_at: Option<DateTime<Utc>>,
	pub views_count: u64,
	pub thumbnail: Option<String>,
}
impl VChapterF {
	pub fn to(value: VChapterF, manga: &Manga) -> Chapter {
		Chapter {
			key: format!("chapter-{}-{}", value.num, value.chapter_number),
			title: value.title,
			chapter_number: Some(value.chapter_number),
			date_uploaded: value.updated_at.or(value.created_at).map(|v| v.timestamp()),
			url: Some(format!(
				"{BASE_URL}/{}/chapter-{}-{}",
				manga.key.replace("/", "/books/"),
				value.num,
				value.chapter_number
			)),
			thumbnail: value.thumbnail,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct Chapters {
	pub chapters: Vec<VChapterF>,
}
