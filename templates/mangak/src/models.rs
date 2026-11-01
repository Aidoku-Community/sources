use crate::helpers::{content_rating, manga_status};
use aidoku::{
	Chapter, Manga, MangaPageResult, MangaWithChapter, Viewer,
	alloc::{String, Vec, format},
	imports::std::parse_date,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApiResponse<T> {
	pub data: T,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct SearchData {
	pub items: Vec<SearchItem>,
	pub pagination: Pagination,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Pagination {
	pub has_next: bool,
}

impl SearchData {
	// The trending feed has no pagination, so it reads as one page
	pub fn into_page(self, base_url: &str) -> MangaPageResult {
		MangaPageResult {
			entries: self
				.items
				.into_iter()
				.map(|item| item.into_manga(base_url))
				.collect(),
			has_next_page: self.pagination.has_next,
		}
	}
}

// The trending feed shares this shape without genres or chapters
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct SearchItem {
	pub name: String,
	pub url: String,
	pub cover: Option<String>,
	pub summary: Option<String>,
	pub status: Option<String>,
	pub genres: Vec<Entity>,
	pub latest_chapters: Vec<ChapterItem>,
}

impl SearchItem {
	pub fn into_manga(self, base_url: &str) -> Manga {
		Manga {
			url: Some(format!("{base_url}{}", self.url)),
			key: self.url,
			title: self.name,
			cover: self.cover,
			..Default::default()
		}
	}

	// Details list genres in the same order
	pub fn into_featured(mut self, base_url: &str) -> Manga {
		let tags: Vec<String> = self.genres.drain(..).map(|genre| genre.name).collect();
		Manga {
			description: self.summary.take().filter(|summary| !summary.is_empty()),
			content_rating: content_rating(&tags),
			status: manga_status(self.status.take()),
			tags: Some(tags).filter(|tags| !tags.is_empty()),
			..self.into_manga(base_url)
		}
	}

	pub fn into_manga_with_chapter(mut self, base_url: &str) -> Option<MangaWithChapter> {
		let chapter = self
			.latest_chapters
			.drain(..)
			.next()?
			.into_chapter(base_url);
		Some(MangaWithChapter {
			manga: self.into_manga(base_url),
			chapter,
		})
	}
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ChapterList {
	pub chapters: Vec<ChapterItem>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ChapterItem {
	pub url: String,
	pub name: String,
	pub updated_at: Option<String>,
}

impl ChapterItem {
	pub fn into_chapter(self, base_url: &str) -> Chapter {
		let chapter_number = self
			.name
			.split_whitespace()
			.nth(1)
			.and_then(|number| number.parse().ok());
		Chapter {
			// Extras and prologues have no number, only a name
			title: chapter_number.is_none().then_some(self.name),
			chapter_number,
			date_uploaded: self
				.updated_at
				.and_then(|date| parse_date(date, "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'")),
			url: Some(format!("{base_url}{}", self.url)),
			key: self.url,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct NextData<T> {
	pub props: Props<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Props<T> {
	pub page_props: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaPage {
	pub initial_manga: MangaDetails,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct MangaDetails {
	pub id: String,
	pub name: String,
	pub summary: Option<String>,
	pub cover: Option<String>,
	pub status: Option<String>,
	pub genres: Option<Vec<Entity>>,
	pub authors: Option<Vec<Entity>>,
	pub artists: Option<Vec<Entity>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Entity {
	pub name: String,
}

impl MangaDetails {
	pub fn into_manga(self, base_url: &str, key: String) -> Manga {
		let names = |list: Option<Vec<Entity>>| {
			list.map(|list| list.into_iter().map(|entity| entity.name).collect())
		};
		let tags: Option<Vec<String>> = names(self.genres);
		Manga {
			content_rating: content_rating(tags.as_deref().unwrap_or_default()),
			url: Some(format!("{base_url}{key}")),
			key,
			title: self.name,
			cover: self.cover,
			description: self.summary,
			authors: names(self.authors),
			artists: names(self.artists),
			tags,
			status: manga_status(self.status),
			viewer: Viewer::Webtoon,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPage {
	pub initial_chapter: ChapterImages,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ChapterImages {
	pub images: Vec<String>,
}
