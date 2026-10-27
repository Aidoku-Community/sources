use crate::helpers::{
	chapter_title, chapter_url, content_rating, cover_url, display_title, is_novel, manga_status,
	manga_url, viewer,
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult,
	alloc::{String, Vec, vec},
};
use serde::Deserialize;

/// Search errors carry a message; the other endpoints don't.
#[derive(Deserialize)]
pub struct ErrorResponse {
	pub message: String,
}

#[derive(Deserialize)]
pub struct FeedResponse {
	pub items: Vec<FeedItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
	pub id: String,
	title: String,
	image: Option<String>,
	medium_image: Option<String>,
	large_image: Option<String>,
	#[serde(default)]
	is_adult: bool,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub medium: Option<String>,
}

impl FeedItem {
	pub fn into_manga(self, label_novels: bool) -> Manga {
		Manga {
			title: display_title(self.title, self.medium.as_deref(), label_novels),
			cover: self
				.medium_image
				.or(self.image)
				.map(|path| cover_url(&path)),
			// The feeds only flag adult titles.
			content_rating: if self.is_adult {
				ContentRating::NSFW
			} else {
				ContentRating::default()
			},
			key: self.id,
			..Default::default()
		}
	}

	pub fn into_featured(self, label_novels: bool, document: Option<SearchDocument>) -> Manga {
		let viewer = viewer(self.kind.as_deref(), self.medium.as_deref());
		let cover = self
			.large_image
			.as_deref()
			.or(self.medium_image.as_deref())
			.or(self.image.as_deref())
			.map(cover_url);
		let manga = self.into_manga(label_novels);
		let Some(document) = document else {
			return Manga {
				viewer,
				cover,
				..manga
			};
		};
		Manga {
			content_rating: document.content_rating(),
			status: manga_status(document.status.as_deref()),
			description: document.synopsis.filter(|synopsis| !synopsis.is_empty()),
			authors: Some(document.authors).filter(|authors| !authors.is_empty()),
			tags: Some(document.tags).filter(|tags| !tags.is_empty()),
			viewer,
			cover,
			..manga
		}
	}
}

#[derive(Deserialize)]
pub struct SearchResponse {
	found: i32,
	page: i32,
	pub hits: Vec<SearchHit>,
	request_params: RequestParams,
}

#[derive(Deserialize)]
struct RequestParams {
	per_page: i32,
}

impl SearchResponse {
	pub fn into_page(self, label_novels: bool) -> MangaPageResult {
		MangaPageResult {
			has_next_page: self.page * self.request_params.per_page < self.found,
			entries: self
				.hits
				.into_iter()
				.map(|hit| hit.document.into_manga(label_novels))
				.collect(),
		}
	}
}

#[derive(Deserialize)]
pub struct SearchHit {
	pub document: SearchDocument,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchDocument {
	pub id: String,
	#[serde(default)]
	title: String,
	poster: Option<String>,
	poster_medium: Option<String>,
	synopsis: Option<String>,
	#[serde(default)]
	authors: Vec<String>,
	/// Genre names.
	#[serde(default)]
	tags: Vec<String>,
	#[serde(default)]
	is_adult: bool,
	mb_content_rating: Option<String>,
	#[serde(default)]
	genre_ids: Vec<String>,
	medium: Option<String>,
	status: Option<String>,
}

impl SearchDocument {
	pub fn content_rating(&self) -> ContentRating {
		content_rating(
			self.is_adult,
			self.mb_content_rating.as_deref(),
			&self.genre_ids,
		)
	}

	pub fn into_manga(self, label_novels: bool) -> Manga {
		let content_rating = self.content_rating();
		Manga {
			title: display_title(self.title, self.medium.as_deref(), label_novels),
			cover: self
				.poster_medium
				.or(self.poster)
				.map(|path| cover_url(&path)),
			status: manga_status(self.status.as_deref()),
			content_rating,
			key: self.id,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResponse {
	// Null for an id the site doesn't know.
	pub manga_page: Option<MangaPage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaPage {
	#[serde(flatten)]
	pub info: MangaInfo,
	#[serde(default)]
	pub chapters: Vec<ChapterItem>,
	#[serde(default)]
	pub scanlators: Vec<Scanlator>,
	#[serde(default)]
	pub has_more_chapters: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaInfo {
	id: String,
	title: String,
	poster: Option<Poster>,
	#[serde(default)]
	authors: Vec<Person>,
	synopsis: Option<String>,
	#[serde(default)]
	genres: Vec<Genre>,
	#[serde(default)]
	tags: Vec<Tag>,
	status: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
	pub medium: Option<String>,
	#[serde(default)]
	is_adult: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Poster {
	image: Option<String>,
	large_image: Option<String>,
}

#[derive(Deserialize)]
struct Person {
	name: String,
	#[serde(rename = "type")]
	role: Option<String>,
}

#[derive(Deserialize)]
struct Genre {
	id: String,
	name: String,
}

#[derive(Deserialize)]
struct Tag {
	name: String,
	weight: Option<String>,
}

impl MangaInfo {
	pub fn into_manga(self, label_novels: bool, rating: Option<SearchDocument>) -> Manga {
		let novel = is_novel(self.medium.as_deref());
		let content_rating = rating.map_or_else(
			|| {
				let genres = self
					.genres
					.iter()
					.map(|genre| genre.id.as_str())
					.collect::<Vec<_>>();
				content_rating(self.is_adult, None, &genres)
			},
			|document| document.content_rating(),
		);
		let (artists, authors): (Vec<_>, Vec<_>) = self
			.authors
			.into_iter()
			.partition(|person| person.role.as_deref() == Some("Artist"));
		let names = |people: Vec<Person>| {
			Some(
				people
					.into_iter()
					.map(|person| person.name)
					.collect::<Vec<_>>(),
			)
			.filter(|names| !names.is_empty())
		};
		// Incidental and recurring tags are too loose to show.
		let tags = self
			.genres
			.into_iter()
			.map(|genre| genre.name)
			.chain(
				self.tags
					.into_iter()
					.filter(|tag| matches!(tag.weight.as_deref(), Some("Core" | "Defining")))
					.map(|tag| tag.name),
			)
			.collect::<Vec<_>>();
		Manga {
			title: display_title(self.title, self.medium.as_deref(), label_novels),
			cover: self
				.poster
				.and_then(|poster| poster.large_image.or(poster.image))
				.map(|path| cover_url(&path)),
			authors: names(authors),
			artists: names(artists),
			description: self.synopsis.filter(|synopsis| !synopsis.is_empty()),
			url: Some(manga_url(&self.id, novel)),
			tags: Some(tags).filter(|tags| !tags.is_empty()),
			status: manga_status(self.status.as_deref()),
			content_rating,
			viewer: viewer(self.kind.as_deref(), self.medium.as_deref()),
			key: self.id,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct Scanlator {
	id: String,
	pub name: String,
	#[serde(default)]
	pub score: i32,
}

#[derive(Deserialize)]
pub struct ChaptersResponse {
	pub chapters: Vec<ChapterItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterItem {
	id: String,
	scanlation_manga_id: Option<String>,
	title: Option<String>,
	pub number: Option<f32>,
	pub created_at: Option<i64>,
	#[serde(default)]
	pub page_count: i32,
}

impl ChapterItem {
	pub fn scanlator<'a>(&self, scanlators: &'a [Scanlator]) -> Option<&'a Scanlator> {
		let id = self.scanlation_manga_id.as_deref()?;
		scanlators.iter().find(|scanlator| scanlator.id == id)
	}

	pub fn into_chapter(self, manga_id: &str, novel: bool, scanlators: &[Scanlator]) -> Chapter {
		let scanlator = self
			.scanlator(scanlators)
			.map(|scanlator| scanlator.name.clone());
		Chapter {
			url: Some(chapter_url(manga_id, &self.id, novel)),
			title: self
				.title
				.as_deref()
				.and_then(|title| chapter_title(title, self.number)),
			chapter_number: self.number,
			date_uploaded: self.created_at.map(|ms| ms / 1000),
			scanlators: scanlator.map(|name| vec![name]),
			key: self.id,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResponse {
	pub read_chapter: ReadChapter,
}

#[derive(Deserialize)]
pub struct ReadChapter {
	pub pages: Vec<PageItem>,
}

#[derive(Deserialize)]
pub struct PageItem {
	pub image: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelResponse {
	pub read_novel_chapter: NovelChapter,
}

#[derive(Deserialize)]
pub struct NovelChapter {
	pub paragraphs: Vec<String>,
}

#[derive(Deserialize)]
pub struct AvailableFilters {
	pub genres: Vec<GenreOption>,
	pub tags: Vec<TagOption>,
}

#[derive(Deserialize)]
pub struct GenreOption {
	pub id: String,
	pub name: String,
}

#[derive(Deserialize)]
pub struct TagOption {
	pub id: String,
	pub name: String,
	pub group: String,
	#[serde(default)]
	pub adult: bool,
}
