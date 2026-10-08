use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LibrarySerie {
	pub title: String,
	pub image: String,
	pub link: String,
	#[serde(rename = "serie_status")]
	pub status: String,
	#[serde(rename = "genres_slugs")]
	pub genres: Vec<String>,
}

#[derive(Deserialize)]
pub struct LibraryMeta {
	pub current_page: i32,
	pub last_page: i32,
}

#[derive(Deserialize)]
pub struct LibrarySeriesWrapper {
	pub data: Vec<LibrarySerie>,
	pub meta: LibraryMeta,
}

#[derive(Deserialize)]
pub struct LibraryProps {
	pub series: LibrarySeriesWrapper,
}

#[derive(Deserialize)]
pub struct InertiaPage<T> {
	pub props: T,
}

#[derive(Deserialize)]
pub struct SerieChapter {
	pub title: String,
	pub slug: String,
	#[serde(rename = "chapterNumber")]
	pub chapter_number: f32,
	#[serde(rename = "createdAt")]
	pub created_at: String,
	pub thumbnail: Option<String>,
}

#[derive(Deserialize)]
pub struct SerieGenre {
	pub name: String,
}

#[derive(Deserialize)]
pub struct SerieDetail {
	pub name: String,
	pub slug: String,
	pub description: String,
	pub author: String,
	pub cover_image: String,
	pub status: String,
	pub genres: Vec<SerieGenre>,
	pub chapters: Vec<SerieChapter>,
}

#[derive(Deserialize)]
pub struct SerieDetailProps {
	pub serie: SerieDetail,
}

#[derive(Deserialize)]
pub struct ChapterSerie {
	pub slug: String,
}

#[derive(Deserialize)]
pub struct ChapterData {
	pub slug: String,
	pub page_count: i32,
	pub chapter_token: String,
	pub serie: ChapterSerie,
}

#[derive(Deserialize)]
pub struct ChapterReaderProps {
	pub data: ChapterData,
}
