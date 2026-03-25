use crate::settings::eng_title;
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
pub struct DesuResponse<T> {
	pub response: Option<T>,
	pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct DesuImage {
	pub original: Option<String>,
	pub preview: Option<String>,
	pub x225: Option<String>,
	pub x120: Option<String>,
	pub x48: Option<String>,
	pub x32: Option<String>,
}

#[derive(Deserialize)]
pub struct DesuDataSummary<T> {
	pub list: Option<Vec<T>>,
}

#[derive(Deserialize)]
pub struct DesuTerm {
	pub id: i32,
	pub text: String,
	pub russian: String,
}

#[derive(Deserialize)]
pub struct DesuAuthor {
	pub people_id: i32,
	pub people_name: String,
}

#[derive(Deserialize)]
pub struct DesuTranslator {
	pub id: i32,
	pub name: String,
	pub site: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct DesuChapter {
	pub id: i64,
	pub vol: Option<f32>,
	pub ch: Option<f32>,
	pub title: Option<String>,
	pub date: Option<i64>,
}

#[derive(Deserialize)]
pub struct DesuPage {
	pub page: i32,
	pub width: i32,
	pub height: i32,
	pub img: Option<String>,
}

#[derive(Deserialize)]
pub struct DesuItem {
	pub id: i64,
	pub url: Option<String>,
	pub name: String,
	pub russian: Option<String>,
	pub image: Option<DesuImage>,
	pub kind: String,
	pub reading: Option<String>,
	pub age_limit: Option<String>,
	pub status: Option<String>,
	pub trans_status: Option<String>,
	pub aired_on: Option<i64>,
	pub released_on: Option<i64>,
	pub score: Option<f32>,
	pub description: Option<String>,
	#[serde(deserialize_with = "deserialize_genres")]
	pub genres: Option<Vec<DesuTerm>>,
	#[serde(deserialize_with = "deserialize_authors")]
	pub authors: Option<Vec<DesuAuthor>>,
	pub translators: Option<Vec<DesuTranslator>>,
	pub chapters: Option<DesuDataSummary<DesuChapter>>,
	pub pages: Option<DesuDataSummary<DesuPage>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawGenres {
	String(String),
	Array(Vec<DesuTerm>),
}

fn deserialize_genres<'de, D>(deserializer: D) -> Result<Option<Vec<DesuTerm>>, D::Error>
where
	D: Deserializer<'de>,
{
	let opt = Option::<RawGenres>::deserialize(deserializer)?;

	Ok(opt.map(|raw| match raw {
		RawGenres::Array(vec) => vec,
		RawGenres::String(s) => s
			.split(',')
			.map(|name| DesuTerm {
				id: 0,
				text: name.trim().to_string(),
				russian: name.trim().to_string(),
			})
			.collect(),
	}))
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawAuthors {
	String(String),
	Array(Vec<DesuAuthor>),
}

fn deserialize_authors<'de, D>(deserializer: D) -> Result<Option<Vec<DesuAuthor>>, D::Error>
where
	D: Deserializer<'de>,
{
	let opt = Option::<RawAuthors>::deserialize(deserializer)?;

	Ok(opt.map(|raw| match raw {
		RawAuthors::Array(vec) => vec,
		RawAuthors::String(s) => s
			.split(',')
			.map(|name| DesuAuthor {
				people_id: 0,
				people_name: name.trim().to_string(),
			})
			.collect(),
	}))
}

impl From<DesuChapter> for Chapter {
	fn from(value: DesuChapter) -> Self {
		Self {
			key: value.id.to_string(),
			volume_number: value.vol,
			chapter_number: value.ch,
			title: value.title,
			date_uploaded: value.date,
			..Default::default()
		}
	}
}

impl From<DesuItem> for Manga {
	fn from(value: DesuItem) -> Self {
		Self {
			key: value.id.to_string(),
			title: if eng_title() {
				value.name
			} else {
				value.russian.unwrap_or(value.name)
			},
			cover: value.image.map(|v| {
				v.original
					.or(v.preview)
					.or(v.x225)
					.or(v.x120)
					.unwrap_or_default()
			}),
			url: value.url,
			description: value.description,
			content_rating: value
				.age_limit
				.map(|v| match v.as_str() {
					// "no" if no age limit. I guess safe by default is ok...
					"18_plus" => ContentRating::NSFW,
					"16_plus" => ContentRating::Suggestive,
					_ => ContentRating::Safe,
				})
				.unwrap_or(ContentRating::Unknown),
			status: value
				.status
				.map(|v| match v.as_str() {
					// looks like they don't have hiatus status and so on
					"ongoing" => MangaStatus::Ongoing,
					"released" => MangaStatus::Completed,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or(MangaStatus::Unknown),
			viewer: match value.kind.as_str() {
				// since they can set read_mode to RTL even for manhwa/manhua I decided to do this
				"manhwa" | "manhua" => Viewer::Webtoon,
				_ => value
					.reading
					.map(|v| match v.as_str() {
						"right-to-left" => Viewer::RightToLeft,
						"left-to-right" => Viewer::LeftToRight,
						"top-to-bottom" => Viewer::Webtoon,
						_ => Viewer::RightToLeft,
					})
					.unwrap_or(Viewer::RightToLeft),
			},
			authors: value
				.authors
				.map(|l| l.into_iter().map(|v| v.people_name).collect()),
			artists: value
				.translators
				.map(|l| l.into_iter().map(|v| v.name).collect()),
			tags: value
				.genres
				.map(|l| l.into_iter().map(|v| v.russian).collect()),
			chapters: value
				.chapters
				.and_then(|s| s.list)
				.map(|l| l.into_iter().map(Chapter::from).collect()),
			..Default::default()
		}
	}
}
