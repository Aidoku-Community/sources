use aidoku::{
	Chapter, Manga,
	alloc::{String, Vec, string::ToString},
	imports::std::parse_date_with_options,
};
use serde::Deserialize;

use crate::{DATE_FORMAT, chapter_url, manga_url};

// `/spa/latest-manga`, `/spa/genre/*` and `/spa/search`
#[derive(Deserialize)]
pub struct MangaListResponse {
	pub manga_list: Vec<MangaEntry>,
	pub pagi: Option<Pagination>,
	// markup of the genre `<select>`, only returned by the genre endpoints
	#[serde(rename = "genreOpt")]
	pub genre_opt: Option<String>,
}

#[derive(Deserialize)]
pub struct Pagination {
	pub button: Option<PaginationButton>,
}

#[derive(Deserialize)]
pub struct PaginationButton {
	// zero on the last page
	pub next: i32,
}

#[derive(Deserialize)]
pub struct MangaEntry {
	pub manga_id: i64,
	pub manga_name: String,
	pub manga_cover_img: Option<String>,
	pub manga_cover_img_full: Option<String>,
}

impl From<MangaEntry> for Manga {
	fn from(value: MangaEntry) -> Self {
		let key = value.manga_id.to_string();
		Manga {
			// the full size cover is preferred, but listings don't always provide it
			cover: value.manga_cover_img_full.or(value.manga_cover_img),
			url: Some(manga_url(&key)),
			title: String::from(value.manga_name.trim()),
			key,
			..Default::default()
		}
	}
}

// `/spa/manga/{manga_id}`. every field is optional: a single unexpected null would otherwise fail
// the whole deserialization, leaving the entry with no chapters at all
#[derive(Deserialize)]
pub struct MangaDetailsResponse {
	#[serde(default)]
	pub detail: Option<MangaEntryDetail>,
	#[serde(default)]
	pub tags: Option<Vec<Tag>>,
	#[serde(default)]
	pub authors: Option<Vec<Author>>,
	#[serde(default)]
	pub chapters: Option<Vec<ChapterEntry>>,
}

#[derive(Deserialize)]
pub struct MangaEntryDetail {
	pub manga_name: Option<String>,
	pub manga_description: Option<String>,
	// true once the series has finished publishing
	pub manga_status: Option<bool>,
	pub manga_cover_img: Option<String>,
	pub manga_cover_img_full: Option<String>,
}

#[derive(Deserialize)]
pub struct Tag {
	pub tag_name: Option<String>,
}

#[derive(Deserialize)]
pub struct Author {
	pub author_name: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterEntry {
	// also the chapter key, since that's what the page endpoint takes
	pub chapter_number: Option<f32>,
	pub chapter_title: Option<String>,
	pub chapter_date_published: Option<String>,
}

impl ChapterEntry {
	// an entry without a number can't be requested
	pub fn into_chapter(self, manga_key: &str) -> Option<Chapter> {
		let number = self.chapter_number?;
		let key = number.to_string();
		let title = self
			.chapter_title
			.map(|title| String::from(title.trim()))
			.filter(|title| !title.is_empty());
		let date_uploaded = self
			.chapter_date_published
			.and_then(|date| parse_date_with_options(date, DATE_FORMAT, "en_US_POSIX", "UTC"));

		// `language` is deliberately left unset: the source is japanese only, so tagging chapters
		// would only expose them to the app's chapter language filter for no benefit
		Some(Chapter {
			url: Some(chapter_url(manga_key, &key)),
			key,
			title,
			chapter_number: Some(number),
			date_uploaded,
			..Default::default()
		})
	}
}

// `/spa/manga/{manga_id}/{chapter_number}`
#[derive(Deserialize)]
pub struct ChapterPagesResponse {
	#[serde(default)]
	pub chapter_detail: Option<ChapterDetail>,
}

#[derive(Deserialize)]
pub struct ChapterDetail {
	// markup holding one lazily loaded `img` per page
	pub chapter_content: Option<String>,
	// base url the page images are relative to, with `servers` holding its mirrors
	pub server: Option<String>,
	#[serde(default)]
	pub slaves: Option<Vec<String>>,
}
