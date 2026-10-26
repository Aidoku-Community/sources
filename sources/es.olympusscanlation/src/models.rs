use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::std::parse_date,
};
use serde::Deserialize;

use crate::helpers::manga_key;

/// Laravel-style pagination used by `rankings`, `new-chapters` and `series`.
#[derive(Deserialize)]
pub struct Paginated<T> {
	pub data: Vec<T>,
	pub current_page: i32,
	pub last_page: i32,
}

impl From<Paginated<MangaListItemDto>> for MangaPageResult {
	fn from(page: Paginated<MangaListItemDto>) -> Self {
		let has_next_page = page.current_page < page.last_page;
		let entries = page
			.data
			.into_iter()
			.filter(MangaListItemDto::is_comic)
			.map(Into::into)
			.collect();
		MangaPageResult {
			entries,
			has_next_page,
		}
	}
}

/// A series as it appears in lists. Its `status` is left out on purpose: some
/// endpoints send it as an object and others as a plain string.
#[derive(Deserialize)]
pub struct MangaListItemDto {
	pub id: i32,
	pub name: String,
	pub slug: String,
	pub cover: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
}

impl MangaListItemDto {
	/// The API mixes novels in with comics; this source only handles comics.
	pub fn is_comic(&self) -> bool {
		self.kind.as_deref() == Some("comic")
	}

	pub fn to_manga(&self) -> Manga {
		Manga {
			key: manga_key(self.id, &self.slug),
			title: self.name.clone(),
			cover: self.cover.clone(),
			..Default::default()
		}
	}
}

impl From<MangaListItemDto> for Manga {
	fn from(dto: MangaListItemDto) -> Self {
		Manga {
			key: manga_key(dto.id, &dto.slug),
			title: dto.name,
			cover: dto.cover,
			..Default::default()
		}
	}
}

/// `/api/series/list`: every series on the site, without pagination.
#[derive(Deserialize)]
pub struct SeriesListResponse {
	pub data: Vec<MangaListItemDto>,
}

/// `/api/series?...` (browsing with filters).
#[derive(Deserialize)]
pub struct SeriesResponse {
	pub data: SeriesData,
}

#[derive(Deserialize)]
pub struct SeriesData {
	pub series: Paginated<MangaListItemDto>,
}

/// An entry of `/api/new-chapters`, which also includes its newest chapters.
#[derive(Deserialize)]
pub struct LatestItemDto {
	pub id: i32,
	pub name: String,
	pub slug: String,
	pub cover: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	#[serde(default)]
	pub last_chapters: Vec<ChapterDto>,
}

/// `/api/series/{slug}`
#[derive(Deserialize)]
pub struct DetailsResponse {
	pub data: MangaDto,
}

#[derive(Deserialize)]
pub struct MangaDto {
	pub id: i32,
	pub slug: String,
	pub name: String,
	pub cover: Option<String>,
	pub summary: Option<String>,
	pub status: Option<StatusDto>,
	pub genres: Option<Vec<GenreDto>>,
}

impl MangaDto {
	/// Fills `manga` with these details. `url` is the series page on the site.
	pub fn into_manga(self, manga: Manga, url: String) -> Manga {
		let status = match self.status.map(|status| status.id) {
			Some(1) => MangaStatus::Ongoing,
			Some(3) => MangaStatus::Hiatus,
			Some(4) => MangaStatus::Completed,
			// 5: cancelled by the author, 7: dropped by the scanlation team
			Some(5 | 7) => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		};

		// Genre names come with trailing spaces ("Acción ").
		let tags: Option<Vec<String>> = self.genres.map(|genres| {
			genres
				.into_iter()
				.map(|genre| genre.name.trim().into())
				.collect()
		});

		let content_rating = if tags
			.as_ref()
			.is_some_and(|tags| tags.iter().any(|tag| tag == "Ecchi"))
		{
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		};

		Manga {
			title: self.name,
			cover: self.cover,
			description: self.summary.map(|summary| summary.trim().into()),
			url: Some(url),
			tags,
			status,
			content_rating,
			// Practically everything on the site is a vertical-strip manhwa.
			viewer: Viewer::Webtoon,
			..manga
		}
	}
}

#[derive(Deserialize)]
pub struct StatusDto {
	pub id: i32,
}

#[derive(Deserialize)]
pub struct GenreDto {
	pub name: String,
}

/// `/api/series/{slug}/chapters`. Unlike the other lists, the page numbers are
/// inside `meta`.
#[derive(Deserialize)]
pub struct ChaptersResponse {
	pub data: Vec<ChapterDto>,
	pub meta: ChaptersMeta,
}

#[derive(Deserialize)]
pub struct ChaptersMeta {
	pub last_page: i32,
}

#[derive(Deserialize)]
pub struct ChapterDto {
	pub id: i32,
	/// Just the chapter number, as text ("185", "10.5").
	pub name: String,
	pub published_at: Option<String>,
	pub team: Option<TeamDto>,
}

impl ChapterDto {
	pub fn into_chapter(self, base_url: &str, slug: &str) -> Chapter {
		Chapter {
			key: self.id.to_string(),
			chapter_number: self.name.trim().parse().ok(),
			date_uploaded: self
				.published_at
				.and_then(|date| parse_date(date, "yyyy-MM-dd'T'HH:mm:ss.SSSSSSZ")),
			url: Some(format!("{base_url}/capitulo/{}/comic-{slug}", self.id)),
			scanlators: self.team.map(|team| vec![team.name.trim().into()]),
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct TeamDto {
	pub name: String,
}
