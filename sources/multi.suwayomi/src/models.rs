use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
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

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MangaDto {
	pub id: u32,
	pub title: String,
	pub thumbnail_url: String,
	pub author: Option<String>,
	pub artist: Option<String>,
	pub genre: Vec<String>,
	pub status: String,
	pub description: String,
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
			description: Some(self.description),
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
	pub id: u32,
	pub name: String,
	pub chapter_number: f32,
	pub scanlator: Option<String>,
	pub upload_date: String,
	pub manga: SlimManga,
}

impl ChapterDto {
	pub fn into_chapter(self, base_url: &str, manga_id: i32) -> Chapter {
		let url = format!("{}/manga/{}/chapter/{}", base_url, manga_id, self.id);

		let scanlator_name = if let Some(ref s) = self.scanlator {
			if !s.is_empty() {
				s
			} else {
				&self.manga.source.display_name
			}
		} else {
			&self.manga.source.display_name
		};
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
