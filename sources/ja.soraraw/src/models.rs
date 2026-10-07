use crate::{
	DATE_FORMAT,
	helpers::{
		authors, chapter_key, chapter_url, contains_ignore_ascii_case, content_rating, cover,
		manga_url, status, strip_html, viewer,
	},
};
use aidoku::{
	Chapter, Manga, MangaPageResult, Viewer,
	alloc::{String, Vec},
	imports::std::parse_date_with_options,
};
use serde::Deserialize;

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

// "/_next/data/soraraw38/index.json"
#[derive(Deserialize)]
pub struct HomeData {
	#[serde(default)]
	pub hot: Vec<MangaEntry>,
	#[serde(default)]
	pub results: Vec<MangaEntry>,
}

// "/top/{period}.json", the ranking lists the site fetches from the browser
#[derive(Deserialize)]
pub struct TopList {
	#[serde(default)]
	pub mangas: Vec<MangaEntry>,
}

// "/newest" and "/genre/{slug}"
#[derive(Deserialize)]
pub struct ListData {
	#[serde(default)]
	pub results: Vec<MangaEntry>,
	pub pagination: Option<Pagination>,
}

impl From<ListData> for MangaPageResult {
	fn from(value: ListData) -> Self {
		MangaPageResult {
			has_next_page: value
				.pagination
				.is_some_and(|pagination| pagination.has_next_page()),
			entries: value.results.into_iter().map(Manga::from).collect(),
		}
	}
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

impl MangaEntry {
	pub fn into_full_manga(self) -> Manga {
		Manga {
			url: Some(manga_url(&self.slug)),
			key: self.slug,
			title: self.name,
			cover: cover(self.thumbnail, self.image.as_deref()),
			authors: authors(self.author.as_deref()),
			status: status(self.kind.as_deref()),
			content_rating: content_rating(self.is_adult.as_deref()),
			..Default::default()
		}
	}
}

impl From<MangaEntry> for Manga {
	fn from(value: MangaEntry) -> Self {
		Manga {
			url: Some(manga_url(&value.slug)),
			key: value.slug,
			title: value.name,
			cover: cover(value.thumbnail, value.image.as_deref()),
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

#[derive(Clone, Deserialize)]
pub struct CatalogueEntry {
	pub name: String,
	pub slug: String,
	pub alt_names: Option<String>,
	pub author: Option<String>,
	#[serde(default)]
	pub genres: Vec<i64>,
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

	pub fn matches_author(&self, needle: &str) -> bool {
		self.author
			.as_deref()
			.is_some_and(|author| contains_ignore_ascii_case(author, needle))
	}
}

impl From<CatalogueEntry> for Manga {
	fn from(value: CatalogueEntry) -> Self {
		Manga {
			url: Some(manga_url(&value.slug)),
			key: value.slug,
			title: value.name,
			cover: cover(None, value.img.as_deref()),
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
		viewer(self.genres.iter().filter_map(|genre| genre.slug.as_deref()))
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
				.filter(|text| {
					!text.is_empty()
						&& !text.starts_with("<b>")
						&& !text.ends_with("RAW FREE")
						&& !text.ends_with("raw FREE")
				})
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

// a few series hold genre rows the site never filled in, every field but the id null
#[derive(Deserialize)]
pub struct Genre {
	pub name: Option<String>,
	// names are not unique, so the reader is picked from the slug
	pub slug: Option<String>,
}

impl Genre {
	pub fn into_tag(self) -> Option<String> {
		let tag = String::from(self.name?.trim());
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
	pub mode: Option<String>,
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
	pub id: i64,
	pub name: String,
	pub slug: String,
}
