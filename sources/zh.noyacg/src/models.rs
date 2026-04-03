use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	prelude::format,
};
use serde::Deserialize;

use crate::helpers::{format_names, get_base_url, get_img_base, split_tags};

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
	pub info: Option<Vec<ListingManga>>,
	pub data: Option<Vec<ListingManga>>,
	pub len: Option<i32>,
}

impl ListingResp {
	pub fn into_page_result(self, page: i32) -> MangaPageResult {
		let entries = self
			.info
			.map(|list| list.into_iter().map(|m| m.into_basic_manga()).collect())
			.unwrap_or_default();
		let total = self.len.unwrap_or(0);
		MangaPageResult {
			has_next_page: page * PAGE_SIZE < total,
			entries,
		}
	}

	pub fn into_manga_chapter_list(self) -> Vec<MangaWithChapter> {
		let Some(list) = self.info else {
			return Vec::new();
		};
		let img_base = get_img_base();
		list.into_iter()
			.map(|m| {
				let key = m.id.to_string();
				let cover = Some(format!("{img_base}/{key}/m1.webp"));
				let status = m.manga_status();
				let content_rating = m.content_rating();
				// chapter name is only available via per-book detail, so use author name as subtitle instead
				let author_name = format_names(&m.author);
				MangaWithChapter {
					manga: Manga {
						title: m.name,
						cover,
						status,
						content_rating,
						key: key.clone(),
						..Default::default()
					},
					chapter: Chapter {
						key,
						title: author_name,
						date_uploaded: Some(m.time),
						..Default::default()
					},
				}
			})
			.collect()
	}

	pub fn into_random_result(self) -> MangaPageResult {
		let entries = self
			.data
			.or(self.info)
			.map(|list| list.into_iter().map(|m| m.into_basic_manga()).collect())
			.unwrap_or_default();
		MangaPageResult {
			has_next_page: false,
			entries,
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListingManga {
	#[serde(rename = "Bid")]
	pub id: i64,
	pub mode: i32,
	#[serde(rename = "Bookname")]
	pub name: String,
	pub description: String,
	pub author: String,
	ptag: String,
	otag: String,
	pub time: i64,
	pub len: i32,
	pub status: i32,
	#[serde(default)]
	pub adult: i32,
}

impl ListingManga {
	pub fn tags(&self) -> Vec<String> {
		let mut tags = split_tags(&self.ptag);
		for tag in split_tags(&self.otag) {
			if !tags.contains(&tag) {
				tags.push(tag);
			}
		}
		tags
	}

	pub fn content_rating(&self) -> ContentRating {
		if self.adult == 1 {
			ContentRating::NSFW
		} else {
			ContentRating::Suggestive
		}
	}

	pub fn is_finished(&self) -> bool {
		self.mode == 0 || self.status == 1
	}

	pub fn manga_status(&self) -> MangaStatus {
		if self.is_finished() {
			MangaStatus::Completed
		} else {
			MangaStatus::Ongoing
		}
	}

	pub fn is_deleted(&self) -> bool {
		self.name == DELETED_TITLE
	}

	pub fn into_basic_manga(self) -> Manga {
		let key = self.id.to_string();
		let img_base = get_img_base();
		Manga {
			cover: Some(format!("{img_base}/{key}/m1.webp")),
			status: self.manga_status(),
			content_rating: self.content_rating(),
			title: self.name,
			key,
			..Default::default()
		}
	}
}

impl From<ListingManga> for Manga {
	fn from(m: ListingManga) -> Self {
		let key = m.id.to_string();
		let img_base = get_img_base();
		let tags = m.tags();
		let status = m.manga_status();
		let content_rating = m.content_rating();
		let authors = format_names(&m.author).map(|a| vec![a]);
		let description = if m.description.is_empty() {
			None
		} else {
			Some(m.description)
		};
		Manga {
			cover: Some(format!("{img_base}/{key}/m1.webp")),
			status,
			content_rating,
			title: m.name,
			authors,
			description,
			tags: Some(tags),
			key,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct FavoritesResp {
	pub data: Option<Vec<ListingManga>>,
	pub count: Option<i32>,
}

impl FavoritesResp {
	pub fn into_page_result(self, page: i32) -> MangaPageResult {
		let entries = self
			.data
			.map(|list| list.into_iter().map(|m| m.into_basic_manga()).collect())
			.unwrap_or_default();
		let total = self.count.unwrap_or(0);
		MangaPageResult {
			has_next_page: page * PAGE_SIZE < total,
			entries,
		}
	}
}

#[derive(Deserialize)]
pub struct SearchResp {
	pub data: Option<Vec<SearchManga>>,
	pub count: Option<i32>,
}

impl SearchResp {
	pub fn into_page_result(self, page: i32) -> MangaPageResult {
		let entries = self
			.data
			.map(|list| list.into_iter().map(Into::into).collect())
			.unwrap_or_default();
		let total = self.count.unwrap_or(0);
		MangaPageResult {
			has_next_page: page * PAGE_SIZE < total,
			entries,
		}
	}
}

#[derive(Deserialize)]
pub struct SearchManga {
	pub id: i64,
	pub name: String,
	pub mode: Option<i32>,
	pub status: Option<i32>,
	#[serde(default)]
	pub adult: Option<i32>,
}

impl From<SearchManga> for Manga {
	fn from(m: SearchManga) -> Self {
		let key = m.id.to_string();
		let img_base = get_img_base();
		let is_finished = m.mode.unwrap_or(1) == 0 || m.status.unwrap_or(0) == 1;
		Manga {
			cover: Some(format!("{img_base}/{key}/m1.webp")),
			title: m.name,
			status: if is_finished {
				MangaStatus::Completed
			} else {
				MangaStatus::Ongoing
			},
			content_rating: if m.adult.unwrap_or(0) == 1 {
				ContentRating::NSFW
			} else {
				ContentRating::Suggestive
			},
			key,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct BookDetailResp {
	pub book: Option<BookWrapper>,
	pub chapters: Option<ChaptersWrapper>,
}

#[derive(Deserialize)]
pub struct BookWrapper {
	pub info: Option<ListingManga>,
}

#[derive(Deserialize)]
pub struct ChaptersWrapper {
	pub categories: Option<Vec<Category>>,
	pub data: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct Category {
	pub id: i64,
	pub name: String,
}

#[derive(Deserialize)]
pub struct ChapterEntry {
	pub id: i64,
	pub name: String,
	pub count: i32,
	pub sort: Option<i32>,
	pub created_at: Option<i64>,
}

impl BookDetailResp {
	pub fn into_manga(self, key: &str) -> Manga {
		let Some(m) = self.book.and_then(|b| b.info) else {
			return Manga {
				key: key.into(),
				..Default::default()
			};
		};
		let tags = m.tags();
		let status = m.manga_status();
		let content_rating = m.content_rating();
		let authors = format_names(&m.author).map(|a| vec![a]);
		let description = if m.description.is_empty() {
			None
		} else {
			Some(m.description)
		};
		let base_url = get_base_url();
		let img_base = get_img_base();
		Manga {
			key: key.into(),
			title: m.name,
			authors,
			description,
			url: Some(format!("{base_url}/manga/{key}")),
			tags: Some(tags),
			cover: Some(format!("{img_base}/{key}/m1.webp")),
			status,
			content_rating,
			// api has no reading direction field; site only hosts manga so RTL is assumed
			viewer: Viewer::RightToLeft,
			..Default::default()
		}
	}

	pub fn take_chapters(&mut self, manga_key: &str) -> Vec<Chapter> {
		let Some(wrapper) = self.chapters.take() else {
			return self.single_chapter_vec(manga_key);
		};

		let Some(categories) = wrapper.categories.filter(|c| !c.is_empty()) else {
			return self.single_chapter_vec(manga_key);
		};

		let Some(serde_json::Value::Object(mut data_map)) = wrapper.data else {
			return self.single_chapter_vec(manga_key);
		};

		let num_categories = categories.len();
		categories
			.iter()
			.rev()
			.enumerate()
			.flat_map(|(cat_idx, category)| {
				let cat_id = category.id.to_string();
				let entries: Vec<ChapterEntry> = data_map
					.remove(&cat_id)
					.and_then(|v| serde_json::from_value(v).ok())
					.unwrap_or_default();

				let volume = (num_categories > 1).then(|| (num_categories - cat_idx) as f32);

				entries.into_iter().rev().map(move |entry| Chapter {
					key: format!("{manga_key}/{}", entry.id),
					title: Some(format!("{}（{}P）", entry.name, entry.count)),
					chapter_number: entry.sort.map(|s| s as f32),
					volume_number: volume,
					scanlators: Some(vec![category.name.clone()]),
					date_uploaded: entry.created_at,
					..Default::default()
				})
			})
			.collect()
	}

	pub fn page_count(&self) -> i32 {
		self.book
			.as_ref()
			.and_then(|b| b.info.as_ref())
			.map(|info| info.len)
			.unwrap_or(0)
	}

	fn single_chapter_vec(&self, manga_key: &str) -> Vec<Chapter> {
		let Some(info) = self.book.as_ref().and_then(|b| b.info.as_ref()) else {
			return Vec::new();
		};
		vec![Chapter {
			key: manga_key.into(),
			title: Some(format!("單章節（{}P）", info.len)),
			chapter_number: Some(1.0),
			date_uploaded: Some(info.time),
			..Default::default()
		}]
	}

	pub fn find_chapter_page_count(&self, chapter_id: &str) -> Option<i32> {
		let wrapper = self.chapters.as_ref()?;
		let data = wrapper.data.as_ref()?;
		wrapper.categories.as_ref()?.iter().find_map(|category| {
			let cat_id = category.id.to_string();
			data.get(&cat_id)?.as_array()?.iter().find_map(|entry| {
				let id = entry.get("id")?.as_i64()?.to_string();
				if id == chapter_id {
					entry.get("count")?.as_i64().map(|c| c as i32)
				} else {
					None
				}
			})
		})
	}
}
