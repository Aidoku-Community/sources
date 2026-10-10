use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Viewer,
	alloc::{String, Vec, collections::BTreeMap, string::ToString, vec},
	prelude::format,
};
use serde::Deserialize;

use crate::WEB_URL;
use crate::helpers::{format_names, get_img_base, parse_chapter_name, split_tags};

const PAGE_SIZE: i32 = 20;
const DELETED_TITLE: &str = "已刪除的内容";

#[derive(Deserialize)]
pub struct LoginResp {
	pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct SigninRecordResp {
	pub today: Option<bool>,
}

#[derive(Deserialize)]
pub struct ListingResp {
	#[serde(alias = "info")]
	data: Option<Vec<ListingManga>>,
	#[serde(alias = "len")]
	count: Option<i32>,
}

impl ListingResp {
	pub fn into_page_result(self, page: i32) -> MangaPageResult {
		let img_base = get_img_base();
		let entries: Vec<Manga> = self
			.data
			.unwrap_or_default()
			.into_iter()
			.map(|m| m.into_basic_manga(&img_base))
			.collect();
		// `len` ignores some filters (e.g. `finished=false`), so also stop at an empty page
		MangaPageResult {
			has_next_page: !entries.is_empty() && page * PAGE_SIZE < self.count.unwrap_or(0),
			entries,
		}
	}
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HomeResp {
	pub tags: Vec<String>,
	pub read_day: Vec<ListingManga>,
	pub fav_day: Vec<ListingManga>,
	pub proportion: Vec<ListingManga>,
	pub fs: Vec<ListingManga>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListingManga {
	#[serde(rename = "Bid")]
	id: i64,
	mode: i32,
	#[serde(rename = "Bookname")]
	name: String,
	description: String,
	author: String,
	ptag: String,
	otag: String,
	time: i64,
	len: i32,
	status: i32,
	#[serde(default)]
	adult: i32,
}

impl ListingManga {
	pub fn is_deleted(&self) -> bool {
		self.name == DELETED_TITLE
	}

	pub fn into_basic_manga(self, img_base: &str) -> Manga {
		let key = self.id.to_string();
		Manga {
			cover: Some(format!("{img_base}/{key}/m1.webp")),
			status: manga_status(self.mode, self.status),
			content_rating: content_rating(self.adult),
			title: self.name,
			key,
			..Default::default()
		}
	}

	fn tags(&self) -> Vec<String> {
		let mut tags = split_tags(&self.ptag);
		for tag in split_tags(&self.otag) {
			if !tags.contains(&tag) {
				tags.push(tag);
			}
		}
		tags
	}
}

#[derive(Deserialize)]
pub struct SearchResp {
	data: Option<Vec<SearchManga>>,
	count: Option<i32>,
}

impl SearchResp {
	pub fn into_page_result(self, page: i32) -> MangaPageResult {
		let img_base = get_img_base();
		MangaPageResult {
			entries: self
				.data
				.unwrap_or_default()
				.into_iter()
				.map(|m| m.into_basic_manga(&img_base))
				.collect(),
			has_next_page: page * PAGE_SIZE < self.count.unwrap_or(0),
		}
	}
}

#[derive(Deserialize)]
struct SearchManga {
	id: i64,
	name: String,
	mode: Option<i32>,
	status: Option<i32>,
	#[serde(default)]
	adult: Option<i32>,
}

impl SearchManga {
	fn into_basic_manga(self, img_base: &str) -> Manga {
		let key = self.id.to_string();
		Manga {
			cover: Some(format!("{img_base}/{key}/m1.webp")),
			title: self.name,
			status: manga_status(self.mode.unwrap_or(1), self.status.unwrap_or(0)),
			content_rating: content_rating(self.adult.unwrap_or(0)),
			key,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct BookDetailResp {
	pub book: Option<BookWrapper>,
	chapters: Option<ChaptersWrapper>,
}

#[derive(Deserialize)]
pub struct BookWrapper {
	pub info: Option<ListingManga>,
}

#[derive(Deserialize)]
struct ChaptersWrapper {
	categories: Option<Vec<Category>>,
	data: Option<BTreeMap<i64, Vec<ChapterEntry>>>,
}

#[derive(Deserialize)]
struct Category {
	id: i64,
	name: String,
}

#[derive(Deserialize)]
struct ChapterEntry {
	id: i64,
	name: String,
	#[serde(default)]
	count: i32,
	created_at: Option<i64>,
}

impl BookDetailResp {
	pub fn into_manga(self, key: &str) -> Option<Manga> {
		let m = self.book?.info?;
		let tags = m.tags();
		Some(Manga {
			key: key.into(),
			cover: Some(format!("{}/{key}/m1.webp", get_img_base())),
			url: Some(format!("{WEB_URL}/manga/{key}")),
			authors: format_names(&m.author).map(|a| vec![a]),
			description: (!m.description.is_empty()).then_some(m.description),
			tags: (!tags.is_empty()).then_some(tags),
			status: manga_status(m.mode, m.status),
			content_rating: content_rating(m.adult),
			// api has no reading direction field; site only hosts manga so RTL is assumed
			viewer: Viewer::RightToLeft,
			title: m.name,
			..Default::default()
		})
	}

	pub fn take_chapters(&mut self, manga_key: &str) -> Vec<Chapter> {
		if let Some(ChaptersWrapper {
			categories: Some(categories),
			data: Some(mut data),
		}) = self.chapters.take()
			&& !categories.is_empty()
		{
			return categories
				.iter()
				.rev()
				.flat_map(|category| {
					// extras are numbered on their own, so their numbers would clash with the main chapters
					let is_extra = category.name.contains("番外");
					data.remove(&category.id)
						.unwrap_or_default()
						.into_iter()
						.rev()
						.map(move |entry| {
							let (chapter_number, volume_number, title) = if is_extra {
								(None, None, Some(entry.name))
							} else {
								parse_chapter_name(entry.name)
							};
							Chapter {
								key: format!("{manga_key}/{}", entry.id),
								title,
								chapter_number,
								volume_number,
								scanlators: Some(vec![category.name.clone()]),
								date_uploaded: entry.created_at,
								..Default::default()
							}
						})
				})
				.collect();
		}
		let Some(info) = self.book.as_ref().and_then(|b| b.info.as_ref()) else {
			return Vec::new();
		};
		vec![Chapter {
			key: manga_key.into(),
			chapter_number: Some(1.0),
			date_uploaded: Some(info.time),
			..Default::default()
		}]
	}

	pub fn page_count(&self, chapter_id: Option<&str>) -> Option<i32> {
		match chapter_id {
			Some(id) => {
				let id: i64 = id.parse().ok()?;
				self.chapters
					.as_ref()?
					.data
					.as_ref()?
					.values()
					.flatten()
					.find(|entry| entry.id == id)
					.map(|entry| entry.count)
			}
			None => self.book.as_ref()?.info.as_ref().map(|info| info.len),
		}
	}
}

fn manga_status(mode: i32, status: i32) -> MangaStatus {
	if mode == 0 || status == 1 {
		MangaStatus::Completed
	} else {
		MangaStatus::Ongoing
	}
}

fn content_rating(adult: i32) -> ContentRating {
	if adult == 1 {
		ContentRating::NSFW
	} else {
		ContentRating::Suggestive
	}
}
