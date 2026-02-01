use crate::{BASE_URL, settings};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Page, PageContent, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchResponse {
	pub result: MangaItems,
}

impl From<SearchResponse> for MangaPageResult {
	fn from(value: SearchResponse) -> Self {
		value.result.into()
	}
}

#[derive(Deserialize)]
pub struct SingleMangaResponse {
	pub result: ComixManga,
}

#[derive(Deserialize)]
pub struct ChapterDetailsResponse {
	pub result: ChapterItems,
}

#[derive(Deserialize)]
pub struct ChapterResponse {
	pub result: Option<ComixChapterWithImages>,
}

#[derive(Deserialize)]
pub struct TermResponse {
	pub result: TermItems,
}

#[derive(Deserialize)]
pub struct Pagination {
	pub current_page: i32,
	pub last_page: i32,
}

#[derive(Deserialize)]
pub struct MangaItems {
	pub items: Vec<ComixManga>,
	pub pagination: Option<Pagination>,
}

impl MangaItems {
	pub fn into_filtered(self, content_types: &[String], hidden_terms: &[i32]) -> MangaPageResult {
		MangaPageResult {
			entries: self
				.items
				.into_iter()
				.filter(|m| !m.is_hidden(content_types, hidden_terms))
				.map(Into::into)
				.collect(),
			has_next_page: self
				.pagination
				.map(|p| p.current_page < p.last_page)
				.unwrap_or_default(),
		}
	}
}

impl From<MangaItems> for MangaPageResult {
	fn from(value: MangaItems) -> Self {
		MangaPageResult {
			entries: value.items.into_iter().map(Into::into).collect(),
			has_next_page: value
				.pagination
				.map(|p| p.current_page < p.last_page)
				.unwrap_or_default(),
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterItems {
	pub items: Vec<ComixChapter>,
	pub pagination: Pagination,
}

#[derive(Deserialize)]
pub struct TermItems {
	pub items: Vec<Term>,
	// pub pagination: Pagination,
}

#[derive(Deserialize)]
pub struct ComixManga {
	pub hash_id: String,
	pub title: String,
	pub synopsis: Option<String>,
	pub r#type: String,
	pub poster: Poster,
	pub status: String,
	pub is_nsfw: bool,
	pub author: Option<Vec<Term>>,
	pub artist: Option<Vec<Term>>,
	pub genre: Option<Vec<Term>>,
	pub latest_chapter: Option<f32>,
	pub chapter_updated_at: Option<i64>,
	pub term_ids: Option<Vec<i32>>,
}

impl ComixManga {
	pub fn is_hidden(&self, hidden_types: &[String], hidden_terms: &[i32]) -> bool {
		if hidden_types.contains(&self.r#type) {
			return true;
		}

		if !hidden_terms.is_empty() {
			self.term_ids
				.as_ref()
				.map(|ids| ids.iter().any(|id| hidden_terms.contains(id)))
				.unwrap_or_default()
		} else {
			false
		}
	}
}

impl From<ComixManga> for Manga {
	fn from(value: ComixManga) -> Self {
		let url = format!("{BASE_URL}/title/{}", value.hash_id);
		Self {
			key: value.hash_id,
			title: value.title,
			cover: match settings::image_quality().as_str() {
				"small" => Some(value.poster.small),
				"medium" => Some(value.poster.medium),
				"large" => Some(value.poster.large),
				_ => None,
			},
			artists: value
				.artist
				.map(|v| v.into_iter().map(|t| t.title).collect()),
			authors: value
				.author
				.map(|v| v.into_iter().map(|t| t.title).collect()),
			description: value.synopsis,
			url: Some(url),
			tags: value
				.genre
				.map(|v| v.into_iter().map(|t| t.title).collect()),
			status: match value.status.as_str() {
				"releasing" => MangaStatus::Ongoing,
				"on_hiatus" => MangaStatus::Hiatus,
				"finished" => MangaStatus::Completed,
				"discontinued" => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			content_rating: if value.is_nsfw {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			},
			viewer: match value.r#type.as_str() {
				"manhwa" => Viewer::Webtoon,
				"manhua" => Viewer::Webtoon,
				"manga" => Viewer::RightToLeft,
				_ => Viewer::Unknown,
			},
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ComixChapter {
	pub chapter_id: i32,
	pub scanlation_group_id: i32,
	pub number: f32,
	pub name: String,
	pub votes: i32,
	pub updated_at: i64,
	pub scanlation_group: Option<ScanlationGroup>,
	pub is_official: i32,
}

impl ComixChapter {
	pub fn into_chapter(self, manga_id: &str) -> Chapter {
		Chapter {
			key: self.chapter_id.to_string(),
			title: (!self.name.is_empty()).then_some(self.name),
			chapter_number: Some(self.number),
			date_uploaded: Some(self.updated_at),
			scanlators: if let Some(scanlation_group) = self.scanlation_group {
				Some(vec![scanlation_group.name])
			} else if self.is_official == 1 {
				Some(vec!["Official".into()])
			} else {
				None
			},
			url: Some(format!("{BASE_URL}/title/{manga_id}/{}", self.chapter_id)),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ComixChapterWithImages {
	// pub chapter_id: i32,
	pub images: Vec<Image>,
}

#[derive(Deserialize)]
pub struct Poster {
	pub small: String,
	pub medium: String,
	pub large: String,
}

#[derive(Deserialize)]
pub struct Term {
	pub term_id: i32,
	pub title: String,
}

#[derive(Deserialize)]
pub struct ScanlationGroup {
	pub name: String,
}

#[derive(Deserialize)]
pub struct Image {
	pub url: String,
}

impl From<Image> for Page {
	fn from(value: Image) -> Self {
		Page {
			content: PageContent::url(value.url),
			..Default::default()
		}
	}
}
