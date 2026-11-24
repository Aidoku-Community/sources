use crate::{
	Params,
	helpers::{chapter_url, upload_page},
};
use aidoku::{
	Chapter, Page, Result,
	alloc::{String, Vec},
	imports::std::parse_date_with_options,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ChapterList {
	pub chapters: Vec<ChapterItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterItem {
	chapter_number: String,
	title: Option<String>,
	slug: String,
	date: Option<String>,
}

#[derive(Deserialize)]
pub struct Genre {
	pub name: String,
	pub slug: String,
}

#[derive(Deserialize)]
pub struct ReaderImage {
	filename: String,
}

impl ReaderImage {
	pub fn into_page(self, base_url: &str) -> Result<Page> {
		upload_page(base_url, &self.filename)
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeaturedSeries {
	pub slug: String,
	pub cover: Option<String>,
	pub content_rating: Option<String>,
}

impl ChapterItem {
	pub fn key(&self) -> &str {
		self.slug.strip_prefix("chapter-").unwrap_or(&self.slug)
	}

	pub fn number(&self) -> Option<f32> {
		self.chapter_number.parse().ok()
	}

	pub fn into_chapter(self, params: &Params, manga_key: &str) -> Option<Chapter> {
		let key = self.key().trim();
		if key.is_empty() {
			return None;
		}
		let key = String::from(key);
		let chapter_number = self.number();
		let date_uploaded = self.date.and_then(|date| {
			parse_date_with_options(date, "yyyy-MM-dd'T'HH:mm:ss.SSSXXX", "en_US_POSIX", "UTC")
		});
		Some(Chapter {
			url: Some(chapter_url(params, manga_key, &key)),
			key,
			title: self.title.and_then(|title| {
				let title = title.trim();
				let label = title
					.strip_prefix("Chapter ")
					.or_else(|| title.strip_prefix("Ch."))
					.unwrap_or(title);
				(!title.is_empty()
					&& label
						.trim()
						.parse::<f32>()
						.ok()
						.is_none_or(|number| Some(number) != chapter_number))
				.then(|| title.into())
			}),
			chapter_number,
			date_uploaded,
			..Default::default()
		})
	}
}
