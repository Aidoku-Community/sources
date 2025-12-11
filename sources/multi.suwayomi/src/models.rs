use aidoku::{
	Chapter, ContentRating, Listing, ListingKind, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, format},
};
use alloc::string::ToString;
use alloc::vec;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GraphQLResponse<T> {
	pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct Nodes<T> {
	pub nodes: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct MultipleMangas {
	pub mangas: Nodes<MangaDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaDto {
	pub id: i32,
	pub title: String,
	pub thumbnail_url: String,
	pub author: Option<String>,
	pub artist: Option<String>,
	pub genre: Vec<String>,
	pub status: String,
}

impl MangaDto {
	pub fn into_manga(self, base_url: &str) -> Manga {
		let url = format!("{}/manga/{}", base_url, self.id);

		let viewer = if self.genre.iter().any(|c| {
			matches!(
				c.to_ascii_lowercase().as_str(),
				"manhwa" | "manhua" | "webtoon"
			)
		}) {
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};

		Manga {
			key: self.id.to_string(),
			title: self.title,
			cover: Some(format!("{}{}", base_url, self.thumbnail_url)),
			artists: self.artist.map(|a| vec![a]),
			authors: self.author.map(|a| vec![a]),
			url: Some(url),
			tags: Some(self.genre),
			status: match self.status.as_str() {
				"ONGOING" => MangaStatus::Ongoing,
				"COMPLETED" => MangaStatus::Completed,
				"CANCELLED" => MangaStatus::Cancelled,
				"ON_HIATUS" => MangaStatus::Hiatus,
				_ => MangaStatus::Unknown,
			},
			content_rating: ContentRating::Safe,
			viewer,
			..Default::default()
		}
	}
}

#[derive(Debug, Deserialize)]
pub struct MultipleChapters {
	pub chapters: Nodes<ChapterDto>,
}

#[derive(Debug, Deserialize)]
pub struct SlimManga {
	pub source: Source,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
	pub display_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
	pub id: i32,
	pub name: String,
	pub chapter_number: f32,
	pub scanlator: Option<String>,
	pub upload_date: String,
	pub manga: SlimManga,
	pub source_order: i32,
}

impl ChapterDto {
	pub fn into_chapter(self, base_url: &str, manga_id: i32) -> Chapter {
		let url = format!(
			"{}/manga/{}/chapter/{}",
			base_url, manga_id, self.source_order
		);

		let scanlator_name = self.scanlator
			.as_ref()
			.filter(|s| !s.is_empty())
			.unwrap_or(&self.manga.source.display_name);
		let scanlator = Some(vec![scanlator_name.clone()]);

		let date_uploaded = self
			.upload_date
			.parse::<i64>()
			.map(|ms| ms / 1000)
			.unwrap_or(0);

		Chapter {
			key: self.id.to_string(),
			title: Some(self.name),
			chapter_number: Some(self.chapter_number),
			date_uploaded: Some(date_uploaded),
			scanlators: scanlator,
			url: Some(url),
			..Default::default()
		}
	}
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchChapterPagesResponse {
	pub fetch_chapter_pages: ChapterPages,
}

#[derive(Debug, Deserialize)]
pub struct ChapterPages {
	pub pages: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MangaOnlyDescriptionResponse {
	pub manga: OnlyDescriptionManga,
}

#[derive(Debug, Deserialize)]
pub struct OnlyDescriptionManga {
	pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct MultipleCategories {
	pub categories: Nodes<CategoryDto>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryDto {
	pub id: i32,
	pub name: String,
}

impl CategoryDto {
	pub fn into_listing(self, total_count: usize) -> Listing {
		// If there are no categories, the category bar isn't visible on the web library view;
		// Rename the listing to "Library"
		let name = if total_count == 1 {
			String::from("Library")
		} else {
			self.name
		};

		Listing {
			id: self.id.to_string(),
			name,
			kind: ListingKind::Default,
		}
	}
}
