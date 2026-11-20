use crate::helpers::{BASE_URL, absolute_url, parse_chapter_name};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Viewer,
	alloc::{String, Vec, format},
	imports::{
		html::{Element, Html},
		std::parse_date,
	},
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

/// The Inertia.js page object stored in the `data-page` attribute of `#app`.
#[derive(Deserialize)]
pub struct InertiaPage {
	pub props: Value,
}

/// Reads one prop from the page, treating a missing or malformed prop as absent.
///
/// Props are read one by one instead of as a single struct so that an unrelated prop
/// changing shape doesn't break every page.
pub fn prop<T: DeserializeOwned>(props: &Value, key: &str) -> Option<T> {
	T::deserialize(props.get(key)?).ok()
}

#[derive(Deserialize)]
pub struct Paginate {
	data: Vec<MangaDto>,
	next_page_url: Option<String>,
}

impl Paginate {
	pub fn into_result(self) -> MangaPageResult {
		MangaPageResult {
			entries: self.data.into_iter().map(MangaDto::into_entry).collect(),
			has_next_page: self.next_page_url.is_some_and(|url| !url.is_empty()),
		}
	}
}

#[derive(Deserialize)]
struct Named {
	name: String,
}

#[derive(Deserialize)]
pub struct MangaDto {
	name: String,
	slug: String,
	cover_url: Option<String>,
	thumb_url: Option<String>,
	pilot: Option<String>,
	description: Option<String>,
	genres: Option<Vec<Named>>,
	artists: Option<Vec<Named>>,
	status_id: Option<i32>,
}

impl MangaDto {
	/// A manga as shown in a listing.
	pub fn into_entry(self) -> Manga {
		Manga {
			url: Some(format!("{BASE_URL}/manga/{}", self.slug)),
			cover: self
				.cover_url
				.or(self.thumb_url)
				.and_then(|url| absolute_url(&url)),
			key: self.slug,
			title: self.name,
			content_rating: ContentRating::NSFW,
			..Default::default()
		}
	}

	/// A manga with all of the details of its own page.
	pub fn into_details(mut self) -> Manga {
		let description = self
			.pilot
			.take()
			.or_else(|| self.description.take())
			.and_then(|html| Element::from(Html::parse_fragment(html).ok()?).text())
			.filter(|text| !text.is_empty());
		let tags = names(self.genres.take());
		let artists = names(self.artists.take());
		let status = match self.status_id {
			Some(0) => MangaStatus::Ongoing,
			Some(1) => MangaStatus::Completed,
			Some(2) => MangaStatus::Hiatus,
			_ => MangaStatus::Unknown,
		};
		let viewer = match &tags {
			Some(tags) if tags.iter().any(|tag| tag.eq_ignore_ascii_case("manga")) => {
				Viewer::RightToLeft
			}
			_ => Viewer::Webtoon,
		};

		Manga {
			description,
			tags,
			// the site only lists a single kind of credit
			authors: artists.clone(),
			artists,
			status,
			viewer,
			..self.into_entry()
		}
	}
}

fn names(list: Option<Vec<Named>>) -> Option<Vec<String>> {
	list.map(|list| list.into_iter().map(|item| item.name).collect::<Vec<_>>())
		.filter(|names| !names.is_empty())
}

#[derive(Deserialize)]
pub struct ChapterDto {
	name: String,
	slug: String,
	created_at: Option<String>,
}

impl ChapterDto {
	pub fn into_chapter(self, manga_key: &str) -> Chapter {
		let (chapter_number, title) = parse_chapter_name(&self.name);
		Chapter {
			url: Some(format!("{BASE_URL}/manga/{manga_key}/{}", self.slug)),
			key: self.slug,
			title,
			chapter_number,
			// timestamps look like `2024-05-01T10:20:30.000000Z`, keep only the part up to the seconds
			date_uploaded: self
				.created_at
				.as_deref()
				.and_then(|date| parse_date(date.get(..19)?, "yyyy-MM-dd'T'HH:mm:ss")),
			..Default::default()
		}
	}
}
