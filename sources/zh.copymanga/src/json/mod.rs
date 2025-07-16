pub mod search;

use super::*;
use aidoku::{MangaStatus, serde::Deserialize};

#[derive(Deserialize)]
pub struct MangaItem {
	path_word: String,
	name: String,
	cover: String,
	status: Option<u8>,
	author: Vec<Author>,
}

impl From<MangaItem> for Manga {
	fn from(item: MangaItem) -> Self {
		let url = Url::manga(&item.path_word).into();

		let key = item.path_word;

		let title = item.name;

		let cover = item.cover.replace(".328x422.jpg", "");

		let authors = item.author.into_iter().map(|author| author.name).collect();

		let status = match item.status {
			Some(0) => MangaStatus::Ongoing,
			Some(1 | 2) => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};

		Self {
			key,
			title,
			cover: Some(cover),
			authors: Some(authors),
			url: Some(url),
			status,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
struct Author {
	name: String,
}
