use crate::{
	helpers::{date, link_keys, novel_title, plain_text, series_key, status},
	markdown,
};
use aidoku::{
	AidokuError, Chapter, Manga, MangaWithChapter, Page, PageContent, Result,
	alloc::{String, Vec, vec},
	prelude::*,
};
use serde::Deserialize;

#[derive(Default, Deserialize)]
pub struct Title {
	#[serde(alias = "manga_title")]
	pub title: String,
	pub cover: String,
	#[serde(default, alias = "manga_url")]
	pub url: String,
	#[serde(default)]
	pub permalink: String,
	#[serde(default)]
	pub manga_permalink: String,
	#[serde(default, alias = "type")]
	pub manga_type: String,
	#[serde(default, alias = "manga_status")]
	pub status: String,
	#[serde(default)]
	pub chapter: String,
	#[serde(default)]
	pub time: String,
}

impl Title {
	pub fn key(&self) -> Option<&str> {
		[&self.manga_permalink, &self.url, &self.permalink]
			.into_iter()
			.find_map(|url| series_key(url))
	}

	pub fn into_update(self) -> Result<MangaWithChapter> {
		let key = link_keys(&self.permalink)
			.and_then(|(_, chapter)| chapter)
			.ok_or(error!("Missing update chapter"))?;
		let chapter_number = self.chapter.trim_start_matches("Ch.").trim().parse().ok();
		let chapter = Chapter {
			key,
			chapter_number,
			date_uploaded: date(&self.time),
			url: Some(self.permalink.clone()),
			..Default::default()
		};
		Ok(MangaWithChapter {
			manga: self.try_into()?,
			chapter,
		})
	}
}

impl TryFrom<Title> for Manga {
	type Error = AidokuError;

	fn try_from(value: Title) -> Result<Self> {
		let key = value.key().ok_or(error!("Missing series key"))?.into();
		let title = plain_text(&value.title)?;
		if title.is_empty() {
			bail!("Missing series title");
		}
		let title = if value.manga_type == "Novel" {
			novel_title(title)
		} else {
			title
		};
		Ok(Manga {
			key,
			title,
			cover: Some(value.cover),
			status: status(&value.status),
			..Default::default()
		})
	}
}

#[derive(Deserialize)]
pub struct MangaFeed {
	pub success: bool,
	pub manga: Vec<Title>,
}

#[derive(Deserialize)]
pub struct PopularChapters {
	pub success: bool,
	pub chapters: Vec<Title>,
}

#[derive(Deserialize)]
pub struct Updates {
	pub success: bool,
	pub data: Vec<Title>,
}

#[derive(Deserialize)]
pub struct Metadata {
	#[serde(rename = "@type")]
	pub kind: String,
	#[serde(default)]
	pub image: Option<String>,
	#[serde(default)]
	pub author: Option<Credit>,
	#[serde(default)]
	pub genre: String,
	#[serde(default)]
	pub status: String,
}

#[derive(Deserialize)]
pub struct Credit {
	pub name: String,
}

#[derive(Deserialize)]
pub struct ChapterList {
	pub success: bool,
	pub chapters: Vec<ChapterItem>,
	pub has_more: bool,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub chapter: String,
	pub title: Option<String>,
	pub date: String,
	pub group_name: Option<String>,
	pub language: String,
	pub url: String,
}

impl TryFrom<ChapterItem> for Chapter {
	type Error = AidokuError;

	fn try_from(value: ChapterItem) -> Result<Self> {
		let key = link_keys(&value.url)
			.and_then(|(_, chapter)| chapter)
			.ok_or(error!("Missing chapter key"))?;
		Ok(Self {
			key,
			title: value
				.title
				.filter(|s| !s.trim().is_empty() && s != "N/A")
				.map(|s| plain_text(&s))
				.transpose()?,
			chapter_number: value.chapter.parse().ok(),
			date_uploaded: date(&value.date),
			scanlators: value
				.group_name
				.filter(|s| !s.trim().is_empty())
				.map(|s| vec![s]),
			url: Some(value.url),
			..Default::default()
		})
	}
}

#[derive(Deserialize)]
pub struct ChapterContent {
	pub success: bool,
	#[serde(default)]
	pub message: Option<String>,
	#[serde(default)]
	pub chapter_type: String,
	#[serde(default)]
	pub images: Vec<String>,
	#[serde(default)]
	pub content: String,
}

impl TryFrom<ChapterContent> for Vec<Page> {
	type Error = AidokuError;

	fn try_from(content: ChapterContent) -> Result<Self> {
		if !content.success {
			bail!(
				"{}",
				content
					.message
					.as_deref()
					.unwrap_or("Chapter is unavailable")
			);
		}
		if content.chapter_type == "text" {
			let text = markdown::parse(&content.content)?;
			if text.trim().is_empty() {
				bail!("Chapter has no text");
			}
			return Ok(vec![Page {
				content: PageContent::text(text),
				..Default::default()
			}]);
		}
		if content.images.is_empty() || content.images.iter().any(|url| url.trim().is_empty()) {
			bail!("Chapter has missing pages");
		}
		Ok(content
			.images
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

#[derive(Deserialize)]
pub struct Slug {
	pub slug: String,
}
