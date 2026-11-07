use crate::{
	BASE_URL,
	helpers::{chapter_url, content_rating, genres, number_and_title},
};
use aidoku::{
	Chapter, Manga, MangaStatus, MangaWithChapter,
	alloc::{String, Vec, string::ToString, vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ErrorResponse {
	pub error: String,
}

#[derive(Deserialize)]
pub struct Novel {
	id: String,
	#[serde(default)]
	title: String,
	author: Option<String>,
	#[serde(default)]
	genres: String,
	cover_url: Option<String>,
	novel_image: Option<String>,
	image_url: Option<String>,
	description: Option<String>,
	release_status: Option<String>,
	total_chapters: Option<String>,
	#[serde(default)]
	chapter_names: Vec<String>, // Only the details endpoint fills it.
	updated_at: Option<String>,
}

#[derive(Deserialize)]
pub struct NovelList {
	pub novels: Vec<Novel>,
	pub pagination: Option<Pagination>,
}

#[derive(Deserialize)]
pub struct Pagination {
	pub has_next: bool,
}

#[derive(Deserialize)]
pub struct NovelDetails {
	pub novel: Novel,
}

#[derive(Deserialize)]
pub struct ChapterResponse {
	pub chapter: ChapterText,
}

#[derive(Deserialize)]
pub struct ChapterText {
	pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct GenreList {
	pub genres: Vec<Genre>,
}

#[derive(Deserialize)]
pub struct Genre {
	pub value: String,
	pub label: String,
}

#[derive(Deserialize)]
pub struct MirrorList {
	pub sources: Vec<Mirror>,
}

// Both a novel's mirrors and the site's whole source list.
#[derive(Deserialize)]
pub struct Mirror {
	pub id: String,
	label: Option<String>,
}

impl Mirror {
	pub fn name(&self) -> String {
		self.label.clone().unwrap_or_else(|| self.id.clone())
	}
}

#[derive(Deserialize)]
pub struct MirrorChapterList {
	pub chapters: Vec<MirrorChapter>,
}

#[derive(Deserialize)]
pub struct MirrorChapter {
	number: f64,
	title: Option<String>,
}

#[derive(Deserialize)]
pub struct MirrorChapterText {
	pub content_html: Option<String>,
}

impl MirrorChapter {
	// `number` addresses the mirror's copy; the title, when it has one, carries the real chapter number.
	pub fn into_chapter(self, novel: &str, mirror: &Mirror) -> Chapter {
		let (number, title) = number_and_title(
			self.title.as_deref().unwrap_or_default(),
			self.number as f32,
		);
		Chapter {
			key: format!("{}:{}", mirror.id, self.number),
			title,
			chapter_number: Some(number),
			url: Some(format!(
				"{}&source={}",
				chapter_url(novel, self.number),
				mirror.id
			)),
			scanlators: Some(vec![mirror.name()]),
			..Default::default()
		}
	}
}

impl Novel {
	// The site falls back through the three image fields in this order.
	fn cover(&self) -> Option<String> {
		let path = [&self.cover_url, &self.novel_image, &self.image_url]
			.into_iter()
			.flatten()
			.map(|path| path.trim())
			.find(|path| !path.is_empty())?;
		Some(if path.starts_with("http") {
			path.into()
		} else {
			format!("{BASE_URL}{path}")
		})
	}

	fn chapter_count(&self) -> usize {
		self.total_chapters
			.as_deref()
			.and_then(|count| count.trim().parse().ok())
			.unwrap_or_default()
	}

	pub fn into_details(self) -> Manga {
		let status = match self
			.release_status
			.as_deref()
			.map(str::to_lowercase)
			.as_deref()
		{
			Some("ongoing") => MangaStatus::Ongoing,
			Some("completed") => MangaStatus::Completed,
			Some("hiatus") => MangaStatus::Hiatus,
			Some("cancelled" | "dropped") => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		};
		let tags = genres(&self.genres);
		Manga {
			cover: self.cover(),
			url: Some(format!("{BASE_URL}/novel?id={}", self.id)),
			title: self.title.trim().into(),
			authors: self
				.author
				.map(|author| author.trim().to_string())
				.filter(|author| !author.is_empty() && author != "Unknown")
				.map(|author| vec![author]),
			description: self
				.description
				.map(|description| description.trim().to_string())
				.filter(|description| !description.is_empty()),
			content_rating: content_rating(&self.genres),
			tags: (!tags.is_empty()).then_some(tags),
			status,
			key: self.id,
			..Default::default()
		}
	}

	// Chapter n is the n-th name; a novel whose names were never collected still has a count.
	pub fn chapters(&self) -> Vec<Chapter> {
		let count = self.chapter_names.len().max(self.chapter_count());
		(1..=count)
			.rev()
			.map(|position| {
				// The number a name prints ("Chapter 165.2"), else the position the reader links.
				let (number, title) = number_and_title(
					self.chapter_names
						.get(position - 1)
						.map_or("", String::as_str),
					position as f32,
				);
				Chapter {
					key: position.to_string(),
					title,
					chapter_number: Some(number),
					url: Some(chapter_url(&self.id, position)),
					..Default::default()
				}
			})
			.collect()
	}

	// The feed names no chapter, so the newest is the last one the novel counts.
	pub fn into_latest(self) -> Option<MangaWithChapter> {
		let number = self.chapter_count();
		if number == 0 {
			return None;
		}
		let date_uploaded = self
			.updated_at
			.as_deref()
			.and_then(|date| parse_date(date, "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'"));
		let url = chapter_url(&self.id, number);
		Some(MangaWithChapter {
			manga: self.into(),
			chapter: Chapter {
				key: number.to_string(),
				chapter_number: Some(number as f32),
				date_uploaded,
				url: Some(url),
				..Default::default()
			},
		})
	}
}

impl From<Novel> for Manga {
	fn from(novel: Novel) -> Self {
		Manga {
			cover: novel.cover(),
			title: novel.title.trim().into(),
			content_rating: content_rating(&novel.genres),
			key: novel.id,
			..Default::default()
		}
	}
}
