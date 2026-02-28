use crate::{
	BASE_URL,
	utils::{capitalize, get_viewer},
};
use aidoku::{
	Chapter, Manga, MangaStatus, MangaWithChapter, Page, PageContent, PageContext,
	alloc::{
		borrow::ToOwned,
		boxed::Box,
		string::{String, ToString},
		vec::Vec,
	},
	prelude::*,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct FeaturedRoot {
	pub data: FeaturedBooksData,
}

#[derive(Debug, Deserialize)]
pub struct SideHomeRoot {
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

	let Some(v) = Option::<Vec<Value>>::deserialize(deserializer).ok() else {
		return Ok(None);
	};

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
pub struct WrapBook {
	pub book: BookItem,
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

	#[serde(default, deserialize_with = "deserialize_tags")]
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
			// chapters: value
			// 	.chapters
			// 	.map(|v| v.into_iter().map(|ref c| c.into()).collect::<Vec<_>>()),
			..Default::default()
		}
	}
}
impl From<BookItem> for MangaWithChapter {
	fn from(value: BookItem) -> Self {
		let chapter = value
			.chapters
			.as_ref()
			.and_then(|v| v.first())
			.map(|v| v.into())
			.unwrap_or_default();
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
	pub created_at: Option<DateTime<Utc>>,
	#[serde(rename = "updatedAt")]
	pub updated_at: Option<DateTime<Utc>>,

	#[serde(rename = "chapterNumber")]
	pub chapter_number: f32,
	pub num: Option<String>,
}
impl From<&VChapter> for Chapter {
	fn from(value: &VChapter) -> Self {
		Self {
			key: format!("chapter-{}-{}", value.chapter_number, value.chapter_number),
			chapter_number: Some(value.chapter_number),
			title: value.num.as_ref().map(|v| format!("Chap {v}")),
			date_uploaded: value.updated_at.or(value.created_at).map(|v| v.timestamp()),
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
#[derive(Deserialize)]
pub struct FlightRoot<T> {
	pub children: T,
}
#[derive(Deserialize)]
#[allow(dead_code)]
pub struct FlightChild<T>(Value, Value, Option<Value>, pub T);

#[derive(Deserialize)]
#[serde(untagged)]
pub enum FlightMutex {
	#[allow(dead_code)]
	Str(String),
	Arr(Box<FlightChild<FlightRoot<FlightChild<FlightNode>>>>),
}

#[derive(Debug, Deserialize)]
pub struct FlightNode {
	pub book: BookItem,
}

#[derive(serde::Deserialize)]
pub struct VChapterF {
	pub num: String,
	#[serde(rename = "chapterNumber")]
	pub chapter_number: f32,
	pub title: Option<String>,
	#[serde(rename = "createdAt")]
	pub created_at: Option<DateTime<Utc>>,
	#[serde(rename = "updatedAt")]
	pub updated_at: Option<DateTime<Utc>>,
	pub thumbnail: Option<String>,
}
impl VChapterF {
	pub fn to(value: VChapterF, manga: &Manga) -> Chapter {
		Chapter {
			key: format!("chapter-{}-{}", value.num, value.chapter_number),
			title: value.title,
			chapter_number: value.num.parse::<f32>().ok(),
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

#[derive(Deserialize)]
pub struct VPage {
	pub width: u32,
	pub height: u32,
	#[serde(rename = "imageUrl")]
	pub image_url: String,
	pub drm_data: Option<String>,
}
impl From<&VPage> for Page {
	fn from(value: &VPage) -> Self {
		let mut context = PageContext::new();
		context.insert("width".to_owned(), value.width.to_string());
		context.insert("height".to_owned(), value.height.to_string());
		context.insert(
			"drm_data".to_owned(),
			value.drm_data.to_owned().unwrap_or_default(),
		);

		Self {
			content: PageContent::url_context(value.image_url.to_owned(), context),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterContent {
	pub cloud: String,
	pub content: Vec<VPage>,
}
