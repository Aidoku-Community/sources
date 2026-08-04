use aidoku::{
	Chapter,
	alloc::{string::String, vec::Vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::Deserialize;

use crate::BASE_URL;

#[derive(Deserialize)]
pub struct ChapterList {
	pub news_id: i32,
	pub chapters: Vec<SingleChapter>,
}

#[derive(Deserialize)]
pub struct SingleChapter {
	date: String,
	id: i32,
	title: String,
}

impl SingleChapter {
	pub fn into_chapter(self, news_id: i32, manga_title: &str) -> Chapter {
		let key = format!("/reader/{}/{}", news_id, self.id);
		let title = self
			.title
			.strip_prefix(manga_title)
			.map(|s| s.trim().into())
			.unwrap_or_else(|| self.title);
		let chapter_number = title
			.find('#')
			.and_then(|idx| title[idx + 1..].parse::<f32>().ok());
		let date_uploaded = parse_date(&self.date, "dd.MM.yyyy");
		let url = format!("{BASE_URL}{key}");
		Chapter {
			key,
			title: Some(title),
			chapter_number,
			date_uploaded,
			url: Some(url),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct PageList {
	pub images: Vec<String>,
}
