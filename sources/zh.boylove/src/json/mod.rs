pub mod chapter_list;
pub mod daily_update;
pub mod manga_page_result;

use super::*;
use aidoku::{ContentRating, MangaStatus, alloc::string::ToString as _, serde::Deserialize};

#[derive(Deserialize)]
struct MangaObj {
	id: u32,
	title: String,
	lanmu_id: Option<u8>,
	image: String,
	auther: String,
	desc: String,
	mhstatus: u8,
	keyword: String,
}

impl From<MangaObj> for Option<Manga> {
	fn from(manga: MangaObj) -> Self {
		if manga.lanmu_id == Some(5) {
			return None;
		}

		let tags = manga
			.keyword
			.split(',')
			.filter(|tag| !tag.is_empty())
			.map(Into::into)
			.collect::<Vec<_>>();

		if tags.iter().any(|tag| tag == "香香公告") {
			return None;
		}

		let key = manga.id.to_string();

		let title = manga.title;

		let cover = if manga.image.starts_with('/') {
			Url::Abs(&manga.image).into()
		} else {
			manga.image
		};

		let authors = manga
			.auther
			.split([',', '&', '/'])
			.filter_map(|author| {
				let trimmed_author = author.trim();
				(!trimmed_author.is_empty()).then(|| trimmed_author.into())
			})
			.collect();

		let description = manga
			.desc
			.trim()
			.replace("\r\n", "\n")
			.replace('\n', "  \n");

		let url = Url::manga(&key).into();

		let status = match manga.mhstatus {
			0 => MangaStatus::Ongoing,
			1 => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};

		let content_rating = if tags.iter().any(|tag| tag == "清水") {
			ContentRating::Safe
		} else {
			ContentRating::NSFW
		};

		Some(Manga {
			key,
			title,
			cover: Some(cover),
			authors: Some(authors),
			description: Some(description),
			url: Some(url),
			tags: Some(tags),
			status,
			content_rating,
			..Default::default()
		})
	}
}
