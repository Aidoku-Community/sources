use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Viewer,
	alloc::{String, Vec, vec},
	imports::{html::Html, std::parse_date},
	prelude::*,
};
use serde::Deserialize;

use crate::BASE_URL;

#[derive(Deserialize)]
pub struct Meta {
	current_page: i32,
	last_page: i32,
}

impl Meta {
	pub fn has_next_page(&self) -> bool {
		self.current_page < self.last_page
	}
}

#[derive(Deserialize)]
pub struct Paginated<T> {
	pub data: Vec<T>,
	pub meta: Meta,
}

#[derive(Deserialize)]
struct Tag {
	name: String,
}

#[derive(Deserialize)]
pub struct Series {
	pub id: i32,
	title: String,
	series_slug: String,
	thumbnail: String,
	description: Option<String>,
	author: Option<String>,
	status: Option<String>,
	tags: Option<Vec<Tag>>,
}

// descriptions are either plain text or html paragraphs
fn parse_description(description: &str) -> Option<String> {
	if !description.contains('<') {
		return Some(description.trim().into());
	}
	let html = Html::parse_fragment(description).ok()?;
	html.select("p")
		.map(|els| {
			els.filter_map(|el| el.text())
				.collect::<Vec<_>>()
				.join("\n\n")
		})
		.filter(|text| !text.is_empty())
		.or_else(|| html.select_first("body")?.text())
}

impl From<Series> for Manga {
	fn from(value: Series) -> Self {
		Manga {
			url: Some(format!("{BASE_URL}/series/{}", value.series_slug)),
			key: value.series_slug,
			title: value.title,
			cover: (!value.thumbnail.is_empty()).then_some(value.thumbnail),
			authors: value
				.author
				.filter(|author| !author.trim().is_empty())
				.map(|author| vec![author.trim().into()]),
			description: value.description.as_deref().and_then(parse_description),
			tags: value
				.tags
				.map(|tags| tags.into_iter().map(|tag| tag.name).collect()),
			status: match value.status.as_deref() {
				Some("Ongoing") => MangaStatus::Ongoing,
				Some("Completed") => MangaStatus::Completed,
				Some("Hiatus") => MangaStatus::Hiatus,
				Some("Dropped") => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			content_rating: ContentRating::NSFW,
			viewer: Viewer::Webtoon,
			..Default::default()
		}
	}
}

impl From<Paginated<Series>> for MangaPageResult {
	fn from(value: Paginated<Series>) -> Self {
		MangaPageResult {
			has_next_page: value.meta.has_next_page(),
			entries: value.data.into_iter().map(Into::into).collect(),
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterItem {
	chapter_name: String,
	chapter_title: Option<String>,
	chapter_slug: String,
	chapter_thumbnail: Option<String>,
	price: Option<i32>,
	created_at: Option<String>,
}

impl ChapterItem {
	pub fn into_chapter(self, series_slug: &str) -> Chapter {
		let chapter_number = self
			.chapter_name
			.strip_prefix("Chapter ")
			.and_then(|number| number.trim().parse().ok());
		Chapter {
			url: Some(format!(
				"{BASE_URL}/series/{series_slug}/{}",
				self.chapter_slug
			)),
			key: self.chapter_slug,
			title: self
				.chapter_title
				.or_else(|| chapter_number.is_none().then_some(self.chapter_name)),
			chapter_number,
			date_uploaded: self
				.created_at
				.and_then(|date| parse_date(date, "yyyy-MM-dd'T'HH:mm:ss.SSSXXX")),
			thumbnail: self.chapter_thumbnail,
			locked: self.price.is_some_and(|price| price > 0),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterResponse {
	pub chapter: ChapterContent,
}

#[derive(Deserialize)]
pub struct ChapterContent {
	pub chapter_data: Option<ChapterData>,
}

#[derive(Deserialize)]
pub struct ChapterData {
	pub images: Vec<String>,
}
