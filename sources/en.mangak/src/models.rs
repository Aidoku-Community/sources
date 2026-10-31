use crate::BASE_URL;
use aidoku::{
	Chapter, Manga, MangaStatus, MangaWithChapter,
	alloc::{String, Vec, format},
	imports::std::parse_date,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApiResponse<T> {
	pub data: T,
}

#[derive(Deserialize)]
pub struct ItemList {
	pub items: Vec<Item>,
}

// The trending feed shares this shape without genres or chapters
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Item {
	pub url: String,
	pub name: String,
	pub cover: Option<String>,
	pub summary: Option<String>,
	pub status: Option<String>,
	pub genres: Vec<Genre>,
	pub latest_chapters: Vec<ChapterItem>,
}

#[derive(Deserialize)]
pub struct Genre {
	pub name: String,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub url: String,
	pub name: String,
	pub updated_at: Option<String>,
}

impl From<Item> for Manga {
	fn from(item: Item) -> Self {
		Manga {
			url: Some(format!("{BASE_URL}{}", item.url)),
			key: item.url,
			title: item.name,
			cover: item.cover,
			..Default::default()
		}
	}
}

impl Item {
	pub fn into_featured(mut self) -> Manga {
		let description = self.summary.take().filter(|summary| !summary.is_empty());
		let status = match self
			.status
			.take()
			.map(|status| status.to_lowercase())
			.as_deref()
		{
			Some("ongoing") => MangaStatus::Ongoing,
			Some("completed") => MangaStatus::Completed,
			Some("hiatus") => MangaStatus::Hiatus,
			Some("cancelled") => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		};
		// Details list genres in the same order
		let tags: Vec<String> = self.genres.drain(..).map(|genre| genre.name).collect();
		Manga {
			description,
			status,
			tags: (!tags.is_empty()).then_some(tags),
			..self.into()
		}
	}

	pub fn into_manga_with_chapter(mut self) -> Option<MangaWithChapter> {
		let chapter = self.latest_chapters.drain(..).next()?;
		Some(MangaWithChapter {
			manga: self.into(),
			chapter: chapter.into(),
		})
	}
}

impl From<ChapterItem> for Chapter {
	fn from(chapter: ChapterItem) -> Self {
		let chapter_number = chapter
			.name
			.split_whitespace()
			.nth(1)
			.and_then(|number| number.parse().ok());
		Chapter {
			// Extras and prologues have no number, only a name
			title: chapter_number.is_none().then_some(chapter.name),
			chapter_number,
			date_uploaded: chapter
				.updated_at
				.and_then(|date| parse_date(date, "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'")),
			url: Some(format!("{BASE_URL}{}", chapter.url)),
			key: chapter.url,
			..Default::default()
		}
	}
}
