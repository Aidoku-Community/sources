use aidoku::{
	Chapter, Manga, Viewer,
	alloc::{String, Vec},
	imports::std::parse_date_with_options,
};
use serde::Deserialize;

use crate::{
	DATE_FORMAT,
	helpers::{
		authors, chapter_key, chapter_url, contains_ignore_ascii_case, content_rating, cover,
		manga_url, status, strip_html, viewer,
	},
};

#[derive(Deserialize)]
pub struct NextData<T> {
	pub props: Props<T>,
}

#[derive(Deserialize)]
pub struct Props<T> {
	#[serde(rename = "pageProps")]
	pub page_props: T,
}

#[derive(Deserialize)]
pub struct DataProps<T> {
	pub data: T,
}

// "/" is the only page holding both the popular and the trending lists
#[derive(Deserialize)]
pub struct HomeProps {
	pub data: ListData,
	#[serde(rename = "initialTrending")]
	pub initial_trending: Option<Trending>,
}

#[derive(Deserialize)]
pub struct Trending {
	#[serde(default)]
	pub mangas: Vec<MangaEntry>,
}

// "/", "/newest" and "/genre/{slug}"
#[derive(Deserialize)]
pub struct ListData {
	// only filled in on the home page
	#[serde(default)]
	pub hot: Vec<MangaEntry>,
	#[serde(default)]
	pub results: Vec<MangaEntry>,
	pub pagination: Option<Pagination>,
}

#[derive(Deserialize)]
pub struct Pagination {
	pub current_page: i32,
	pub total_page: i32,
}

impl Pagination {
	pub fn has_next_page(&self) -> bool {
		self.current_page < self.total_page
	}
}

// listings don't all carry the same fields, so everything but the two a cover needs is optional
#[derive(Deserialize)]
pub struct MangaEntry {
	pub name: String,
	pub slug: String,
	pub author: Option<String>,
	pub image: Option<String>,
	pub thumbnail: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub is_adult: Option<String>,
}

impl From<MangaEntry> for Manga {
	fn from(value: MangaEntry) -> Self {
		// `viewer` is left unset: listings carry no genres, and the reader is picked from those.
		// the app fills it in from `get_manga_update` before a chapter can be opened anyway
		Manga {
			cover: cover(value.thumbnail, value.image.as_deref()),
			title: value.name.trim().into(),
			authors: authors(value.author.as_deref()),
			url: Some(manga_url(&value.slug)),
			key: value.slug,
			status: status(value.kind.as_deref()),
			content_rating: content_rating(value.is_adult.as_deref()),
			..Default::default()
		}
	}
}

// "/mangas_{n}.json", the catalogue dump the site searches through in the browser
#[derive(Deserialize)]
pub struct CataloguePage {
	#[serde(default)]
	pub list: Vec<CatalogueEntry>,
}

#[derive(Deserialize)]
pub struct CatalogueEntry {
	pub name: String,
	pub slug: String,
	pub alt_names: Option<String>,
	pub author: Option<String>,
	// the cover file name, called "image" everywhere else
	pub img: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub is_adult: Option<String>,
}

impl CatalogueEntry {
	// the same three fields the site's own search runs over. its fuzzy matcher isn't reproduced:
	// a plain substring match keeps the walk over 24k entries allocation free
	pub fn matches(&self, needle: &str) -> bool {
		[
			Some(self.name.as_str()),
			self.alt_names.as_deref(),
			self.author.as_deref(),
		]
		.into_iter()
		.flatten()
		.any(|field| contains_ignore_ascii_case(field, needle))
	}

	// for the search field "supportsAuthorSearch" enables
	pub fn matches_author(&self, needle: &str) -> bool {
		self.author
			.as_deref()
			.is_some_and(|author| contains_ignore_ascii_case(author, needle))
	}
}

impl From<CatalogueEntry> for Manga {
	fn from(value: CatalogueEntry) -> Self {
		// the dump gives genres as bare ids, which would need the genre index to resolve, so the
		// reader is left to the details request like it is for the listings
		Manga {
			cover: cover(None, value.img.as_deref()),
			title: value.name.trim().into(),
			authors: authors(value.author.as_deref()),
			url: Some(manga_url(&value.slug)),
			key: value.slug,
			status: status(value.kind.as_deref()),
			content_rating: content_rating(value.is_adult.as_deref()),
			..Default::default()
		}
	}
}

// "/manga/{slug}"
#[derive(Deserialize)]
pub struct MangaData {
	pub manga: Option<MangaDetails>,
}

#[derive(Deserialize)]
pub struct MangaDetails {
	pub id: i64,
	pub name: String,
	pub slug: String,
	pub author: Option<String>,
	pub image: Option<String>,
	// always null in practice; the synopsis lives in "content" as an Editor.js document
	pub description: Option<String>,
	pub content: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub is_adult: Option<String>,
	#[serde(default)]
	pub genres: Vec<Genre>,
	#[serde(default)]
	pub chapters: Vec<ChapterEntry>,
}

impl MangaDetails {
	pub fn cover(&self) -> Option<String> {
		cover(None, self.image.as_deref())
	}

	pub fn viewer(&self) -> Viewer {
		viewer(self.genres.iter().map(|genre| genre.slug.as_str()))
	}

	pub fn authors(&self) -> Option<Vec<String>> {
		authors(self.author.as_deref())
	}

	pub fn description(&self) -> Option<String> {
		if let Some(description) = self.description.as_deref().map(strip_html)
			&& !description.is_empty()
		{
			return Some(description);
		}

		let document = serde_json::from_str::<EditorDocument>(self.content.as_deref()?).ok()?;
		let mut description = String::new();
		for block in &document.blocks {
			let Some(text) = block
				.data
				.as_ref()
				.and_then(|data| data.text.as_deref())
				.map(strip_html)
				.filter(|text| !text.is_empty())
			else {
				continue;
			};
			if !description.is_empty() {
				description.push_str("\n\n");
			}
			description.push_str(&text);
		}

		(!description.is_empty()).then_some(description)
	}
}

#[derive(Deserialize)]
pub struct Genre {
	pub name: String,
	// names are not unique, so the reader is picked from the slug
	pub slug: String,
}

impl Genre {
	pub fn into_tag(self) -> Option<String> {
		let tag = String::from(self.name.trim());
		(!tag.is_empty()).then_some(tag)
	}
}

#[derive(Deserialize)]
pub struct EditorDocument {
	#[serde(default)]
	pub blocks: Vec<EditorBlock>,
}

#[derive(Deserialize)]
pub struct EditorBlock {
	pub data: Option<EditorBlockData>,
}

#[derive(Deserialize)]
pub struct EditorBlockData {
	pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterEntry {
	pub id: i64,
	pub name: Option<Number>,
	pub title: Option<String>,
	pub path: String,
	pub published_at: Option<String>,
}

impl ChapterEntry {
	pub fn into_chapter(self, manga_id: i64, manga_slug: &str) -> Chapter {
		Chapter {
			key: chapter_key(manga_id, self.id),
			title: self
				.title
				.map(|title| String::from(title.trim()))
				.filter(|title| !title.is_empty()),
			chapter_number: self.name.as_ref().and_then(Number::as_f32),
			date_uploaded: self
				.published_at
				.and_then(|date| parse_date_with_options(date, DATE_FORMAT, "en_US_POSIX", "UTC")),
			url: Some(chapter_url(manga_slug, &self.path)),
			// `language` is deliberately left unset: the source is japanese only, so tagging
			// chapters would only expose them to the app's chapter language filter for no benefit
			..Default::default()
		}
	}
}

// "/manga/{slug}/{chapter}", read only to resolve deep links
#[derive(Deserialize)]
pub struct ChapterData {
	pub chapter: Option<ChapterDetails>,
}

#[derive(Deserialize)]
pub struct ChapterDetails {
	pub id: i64,
	pub manga_id: i64,
	// the image paths are encrypted with this, and only the chapter page carries it
	pub uuid: Option<String>,
	#[serde(rename = "_b")]
	pub base: Option<String>,
}

#[derive(Deserialize)]
pub struct ImagePayload {
	pub d: String,
}

#[derive(Deserialize)]
pub struct PageImage {
	pub order: Number,
	// the encrypted image path. entries also carry a `d` naming the same file on the google drive
	// mirror, which is left unread: not every entry holds one
	pub b: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Number {
	Float(f32),
	Text(String),
}

impl Number {
	pub fn as_f32(&self) -> Option<f32> {
		match self {
			Number::Float(value) => Some(*value),
			Number::Text(value) => value.trim().parse().ok(),
		}
	}
}

// "/genres.json"
#[derive(Deserialize)]
pub struct GenreEntry {
	pub name: String,
	pub slug: String,
}
