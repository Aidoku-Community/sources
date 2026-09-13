use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Page, PageContent, Viewer,
	alloc::{String, Vec},
	imports::{html::Html, std::parse_date},
	prelude::*,
};
use serde::Deserialize;

use crate::{BASE_URL, CHAPTERS_PATH, SERIES_PATH};

#[derive(Deserialize)]
struct BrowseTag {
	r#type: String,
	name: String,
}

#[derive(Deserialize)]
pub struct MangaChapterHeader {
	#[allow(dead_code)]
	header: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	title: String,
	permalink: String,
	released_on: String,
	tags: Vec<BrowseTag>,
}

impl ChapterItem {
	fn chapter_number(&self) -> Option<f32> {
		let (_, chapter) = self.permalink.rsplit_once("_ch")?;

		match chapter.split_once('_') {
			Some((whole, fraction))
				if whole.chars().all(|c| c.is_ascii_digit())
					&& fraction.chars().all(|c| c.is_ascii_digit()) =>
			{
				format!("{whole}.{fraction}").parse().ok()
			}
			None if chapter.chars().all(|c| c.is_ascii_digit()) => chapter.parse().ok(),
			_ => None,
		}
	}
}

impl From<ChapterItem> for Chapter {
	fn from(value: ChapterItem) -> Self {
		let url = format!("{BASE_URL}{CHAPTERS_PATH}{}", value.permalink);
		let chapter_number = value.chapter_number();
		let title = if let Some(chapter_number) = chapter_number {
			let result = value
				.title
				.trim_start_matches(&format!("Chapter {chapter_number}"))
				.trim_start_matches(":")
				.trim();
			(!result.is_empty()).then_some(result.into())
		} else {
			Some(value.title)
		};
		Chapter {
			key: value.permalink,
			title,
			chapter_number,
			date_uploaded: parse_date(value.released_on, "yyyy-MM-dd"),
			scanlators: Some(
				value
					.tags
					.into_iter()
					.filter(|t| t.r#type == "Scanlator")
					.map(|t| t.name)
					.collect(),
			),
			url: Some(url),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Tagging {
	Chapter(ChapterItem),
	#[allow(dead_code)]
	Header(MangaChapterHeader),
}

#[derive(Deserialize)]
pub struct MangaEntry {
	name: String,
	permalink: String,
	cover: Option<String>,
	tags: Vec<BrowseTag>,
	description: Option<String>,
	taggings: Option<Vec<Tagging>>,
}

impl MangaEntry {
	pub fn chapters(&mut self) -> Option<Vec<Chapter>> {
		let taggings = self.taggings.take()?;
		Some(
			taggings
				.into_iter()
				.filter_map(|t| match t {
					Tagging::Chapter(c) => Some(Chapter::from(c)),
					Tagging::Header(_) => None,
				})
				.rev()
				.collect(),
		)
	}

	pub fn cover(&self) -> Option<String> {
		self.cover
			.as_ref()
			.map(|cover| format!("{BASE_URL}{}", cover))
	}
}

impl From<MangaEntry> for Manga {
	fn from(value: MangaEntry) -> Self {
		let cover = value.cover();

		let mut authors: Vec<String> = Vec::new();
		let mut tags: Vec<String> = Vec::new();
		let mut status = MangaStatus::Unknown;
		let mut content_rating = ContentRating::Safe;

		for tag in value.tags {
			match tag.r#type.as_str() {
				"Author" => authors.push(tag.name),
				"General" => {
					if tag.name == "NSFW" || tag.name.contains("sex") {
						content_rating = ContentRating::NSFW;
					} else if content_rating == ContentRating::Safe && tag.name == "Ecchi" {
						content_rating = ContentRating::Suggestive;
					}
					tags.push(tag.name);
				}
				"Status" => {
					status = match tag.name.as_str() {
						"Ongoing" => MangaStatus::Ongoing,
						"Completed" => MangaStatus::Completed,
						"On Hiatus" => MangaStatus::Hiatus,
						"Licensed" => MangaStatus::Cancelled,
						"Dropped" | "Cancelled" | "Not Updated" | "Abandoned" | "Removed" => {
							MangaStatus::Cancelled
						}
						_ => continue,
					}
				}
				_ => continue,
			}
		}

		let url = format!("{BASE_URL}{SERIES_PATH}{}", value.permalink);

		Manga {
			key: value.permalink,
			title: value.name,
			cover,
			authors: (!authors.is_empty()).then_some(authors),
			description: value
				.description
				.and_then(|d| Html::parse_fragment(d).ok()?.select_first("body")?.text()),
			url: Some(url),
			tags: (!tags.is_empty()).then_some(tags),
			status,
			content_rating,
			viewer: Viewer::RightToLeft,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterResponse {
	// title: String,
	// permalink: String,
	// tags: Vec<BrowseTag>,
	pages: Vec<ChapterPage>,
}

#[derive(Deserialize)]
pub struct ChapterPage {
	url: String,
}

impl ChapterResponse {
	pub fn pages(self) -> Vec<Page> {
		self.pages
			.into_iter()
			.map(|p| Page {
				content: PageContent::url(format!("{BASE_URL}{}", p.url)),
				..Default::default()
			})
			.collect()
	}
}
