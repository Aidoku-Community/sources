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
	#[serde(default, deserialize_with = "deserialize_genres")]
	pub genres: Option<Vec<DesuTerm>>,
	#[serde(default, deserialize_with = "deserialize_authors")]
	pub authors: Option<Vec<DesuAuthor>>,
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
	Ok(Option::<RawAuthors>::deserialize(deserializer)?
		.map(|raw| match raw {
			RawAuthors::Array(vec) => vec,
			RawAuthors::String(s) => s
				.split(',')
				.map(|name| name.trim())
				.filter(|name| !name.is_empty())
				.map(|name| DesuAuthor {
					people_id: 0,
					people_name: name.to_string(),
				})
				.collect(),
		})
		.filter(|l| !l.is_empty()))
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

impl DesuItem {
	pub fn get_key(&self) -> String {
		self.id.to_string()
	}

	pub fn get_title(&self) -> String {
		if eng_title() {
			self.name.clone()
		} else {
			self.russian.clone().unwrap_or_else(|| self.name.clone())
		}
	}

	pub fn get_cover(&self) -> Option<String> {
		self.image.as_ref().and_then(|v| {
			v.original
				.clone()
				.or_else(|| v.preview.clone())
				.or_else(|| v.x225.clone())
				.or_else(|| v.x120.clone())
		})
	}

	pub fn get_status(&self) -> MangaStatus {
		self.status
			.as_ref()
			.map(|v| match v.as_str() {
				// looks like they don't have hiatus status and so on
				"ongoing" => MangaStatus::Ongoing,
				"released" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			})
			.unwrap_or_default()
	}

	pub fn get_rating(&self) -> ContentRating {
		self.age_limit
			.as_ref()
			.map(|v| match v.as_str() {
				// "no" if no age limit. I guess safe by default is ok...
				"18_plus" => ContentRating::NSFW,
				"16_plus" => ContentRating::Suggestive,
				_ => ContentRating::Safe,
			})
			.unwrap_or_default()
	}

	pub fn get_viewer(&self) -> Viewer {
		match self.kind.as_ref() {
			// since they can set read_mode to RTL even for manhwa/manhua I decided to do this
			"manhwa" | "manhua" => Viewer::Webtoon,
			_ => self
				.reading
				.as_ref()
				.map(|v| match v.as_str() {
					"right-to-left" => Viewer::RightToLeft,
					"left-to-right" => Viewer::LeftToRight,
					"top-to-bottom" => Viewer::Webtoon,
					_ => Viewer::RightToLeft,
				})
				.unwrap_or(Viewer::RightToLeft),
		}
	}

	pub fn get_authors(&self) -> Option<Vec<String>> {
		self.authors
			.as_ref()
			.map(|l| l.iter().map(|v| v.people_name.clone()).collect())
	}

	pub fn get_tags(&self) -> Option<Vec<String>> {
		self.genres
			.as_ref()
			.map(|l| l.iter().map(|v| v.russian.clone()).collect())
	}

	pub fn get_chapters(&self) -> Option<Vec<Chapter>> {
		self.chapters
			.as_ref()
			.and_then(|s| s.list.as_ref())
			.map(|l| l.iter().map(|x| Chapter::from(x.clone())).collect())
	}

	pub fn to_slim_item(&self) -> Manga {
		Manga {
			key: self.get_key(),
			title: self.get_title(),
			cover: self.get_cover(),
			..Default::default()
		}
	}

	pub fn to_manga(&self, details: bool, chapters: bool) -> Manga {
		Manga {
			// those data always copied
			key: self.get_key(),
			title: self.get_title(),
			content_rating: self.get_rating(),
			status: self.get_status(),
			viewer: self.get_viewer(),
			// those should be copied only if required
			cover: if details { self.get_cover() } else { None },
			authors: if details { self.get_authors() } else { None },
			description: if details && self.description.is_some() {
				self.description.clone()
			} else {
				None
			},
			url: if details && self.url.is_some() {
				self.url.clone()
			} else {
				None
			},
			tags: if details { self.get_tags() } else { None },
			chapters: if chapters { self.get_chapters() } else { None },
			..Default::default()
		}
	}
}
