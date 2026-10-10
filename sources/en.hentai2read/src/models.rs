use crate::{BASE_URL, TAG_KINDS, absolute_url};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, UpdateStrategy, Viewer,
	alloc::{String, Vec, string::ToString},
	imports::std::parse_date_with_options,
	prelude::*,
};
use serde::Deserialize;

/// The Inertia.js page object embedded in every page as `<script data-page="app">`.
#[derive(Deserialize)]
pub struct InertiaPage<T> {
	pub props: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProps {
	pub books: Paginated,
	/// Only on search pages: tags, artists and parodies whose names match the query.
	#[serde(default)]
	pub matching_tags: Vec<TagRef>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paginated {
	pub data: Vec<BookItem>,
	pub page: i32,
	pub last_page: i32,
	#[serde(default)]
	pub total: i32,
}

impl From<Paginated> for MangaPageResult {
	fn from(books: Paginated) -> Self {
		MangaPageResult {
			has_next_page: books.page < books.last_page,
			entries: books
				.data
				.into_iter()
				.filter_map(BookItem::into_manga)
				.collect(),
		}
	}
}

#[derive(Deserialize)]
pub struct BookItem {
	pub title: String,
	pub url: String,
	pub cover: Option<String>,
	#[serde(rename = "type")]
	pub kind: String,
}

impl BookItem {
	/// Videos can't be read, so they are skipped.
	pub fn into_manga(self) -> Option<Manga> {
		if self.kind == "video" {
			return None;
		}
		Some(Manga {
			url: Some(format!("{BASE_URL}{}", self.url)),
			key: self.url,
			title: self.title,
			cover: self.cover,
			content_rating: ContentRating::NSFW,
			..Default::default()
		})
	}
}

#[derive(Deserialize)]
pub struct BookProps {
	pub book: Book,
	pub chapters: Vec<BookChapter>,
	#[serde(default)]
	pub stats: Stats,
}

#[derive(Deserialize, Default)]
pub struct Stats {
	pub views: Option<i64>,
	/// Average out of 5.
	pub rating: Option<f32>,
	pub ratings: Option<i64>,
	pub rank: Option<i64>,
}

#[derive(Deserialize)]
pub struct Book {
	pub title: String,
	pub cover: Option<String>,
	pub alt: Option<String>,
	pub year: Option<i32>,
	pub status: Option<Label>,
	pub description: Option<String>,
	pub facts: Option<String>,
	pub tags: Vec<Tag>,
	pub groups: Vec<TagGroup>,
}

#[derive(Deserialize)]
pub struct Label {
	pub label: String,
}

#[derive(Deserialize)]
pub struct Tag {
	pub name: String,
	#[serde(default)]
	pub url: String,
}

#[derive(Deserialize)]
pub struct TagGroup {
	pub label: String,
	#[serde(rename = "type")]
	pub kind: String,
	pub tags: Vec<Tag>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookChapter {
	pub number: String,
	pub title: Option<String>,
	pub url: String,
	pub date: Option<String>,
	pub is_members: bool,
}

fn escape_markdown(text: &str) -> String {
	let mut escaped = String::with_capacity(text.len());
	for char in text.chars() {
		if matches!(char, '\\' | '[' | ']' | '*' | '_' | '`') {
			escaped.push('\\');
		}
		escaped.push(char);
	}
	escaped
}

/// Formats a number with thousands separators, e.g. `1954335` as `1,954,335`.
fn thousands(number: i64) -> String {
	let digits = number.to_string();
	let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
	for (index, digit) in digits.chars().enumerate() {
		if index > 0 && (digits.len() - index).is_multiple_of(3) {
			formatted.push(',');
		}
		formatted.push(digit);
	}
	formatted
}

impl Book {
	pub fn apply_to(self, stats: Stats, manga: &mut Manga) {
		// Shown at the end of the description, one per line, when the site has them.
		let mut info = Vec::new();
		if let Some(alt) = self.alt.as_deref().filter(|alt| !alt.is_empty()) {
			info.push(format!("Alternative title: {}", escape_markdown(alt)));
		}
		if let Some(year) = self.year {
			info.push(format!("Year: {year}"));
		}
		if let Some(rating) = stats.rating.filter(|rating| *rating > 0.0) {
			let count = stats.ratings.unwrap_or_default();
			let noun = if count == 1 { "rating" } else { "ratings" };
			info.push(format!("Rating: {rating}/5 ({} {noun})", thousands(count)));
		}
		if let Some(views) = stats.views.filter(|views| *views > 0) {
			info.push(format!("Views: {}", thousands(views)));
		}
		if let Some(rank) = stats.rank {
			info.push(format!("Rank: #{}", thousands(rank)));
		}

		let mut artists = Vec::new();
		let mut parodies = Vec::new();
		// Descriptions are rendered as markdown, and site links open as listings through the deep
		// link handler, so each artist, parody, character and publisher is linked.
		for group in self.groups {
			if group.kind == "language" {
				continue;
			}
			let links: Vec<String> = group
				.tags
				.iter()
				.map(|tag| {
					let name = escape_markdown(&tag.name);
					// Urls come from site data, so only tag page paths are linked.
					if is_tag_page(&tag.url) {
						format!("[{name}]({BASE_URL}{})", tag.url)
					} else {
						name
					}
				})
				.collect();
			info.push(format!("{}: {}", group.label, links.join(", ")));

			let names = group.tags.into_iter().map(|tag| tag.name);
			match group.kind.as_str() {
				"artist" => artists.extend(names),
				"parody" => parodies.extend(names),
				_ => {}
			}
		}
		let is_webtoon = self.tags.iter().any(|tag| tag.name == "Korean Comic");
		// Regular tags come first, followed by artists and parodies. Tapped tags are sent by name
		// only, so the search resolves which kind of tag each one is.
		let mut tags: Vec<String> = self.tags.into_iter().map(|tag| tag.name).collect();
		// The app identifies tags by name, so duplicates are skipped.
		for name in artists.iter().chain(&parodies) {
			if !tags.contains(name) {
				tags.push(name.into());
			}
		}

		let mut description = self.description.or(self.facts).unwrap_or_default();
		if !info.is_empty() {
			if !description.is_empty() {
				description.push_str("\n\n");
			}
			// Markdown needs two trailing spaces for a line break.
			description.push_str(&info.join("  \n"));
		}

		manga.viewer = if is_webtoon {
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};
		manga.status = match self.status.as_ref().map(|status| status.label.as_str()) {
			Some("Ongoing") => MangaStatus::Ongoing,
			Some("Completed") => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};
		// Completed series don't get new chapters, so library updates can skip them.
		manga.update_strategy = if manga.status == MangaStatus::Completed {
			UpdateStrategy::Never
		} else {
			UpdateStrategy::Always
		};
		manga.title = self.title;
		manga.cover = self.cover;
		manga.description = (!description.is_empty()).then_some(description);
		manga.authors = Some(artists);
		manga.tags = Some(tags);
		manga.content_rating = ContentRating::NSFW;
	}
}

impl BookChapter {
	pub fn into_chapter(self, book_title: &str) -> Chapter {
		// The key is the reader base path, e.g. `/reader/72281-flagship/115609`.
		let key = match self.url.rfind("/page-") {
			Some(index) => String::from(&self.url[..index]),
			None => self.url,
		};
		Chapter {
			url: Some(absolute_url(&key)),
			title: self.title.filter(|title| title != book_title),
			chapter_number: self.number.parse().ok(),
			date_uploaded: self.date.and_then(|date| {
				// Dates have no time, so parse them as local days to avoid showing the previous day.
				parse_date_with_options(date, "MMM d, yyyy", "en_US_POSIX", "current")
			}),
			locked: self.is_members,
			key,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ReaderProps {
	pub book: ReaderBook,
	pub images: Vec<String>,
}

#[derive(Deserialize)]
pub struct ReaderBook {
	pub url: String,
}

/// A tag, artist, parody or character, from the artist autocomplete or search results.
#[derive(Deserialize)]
pub struct TagRef {
	pub id: i32,
	pub name: String,
}

/// Checks that a url is a tag page path like `/parody/751-kantai-collection`: a known tag kind,
/// a numeric id and a slug of letters, digits and dashes.
pub fn is_tag_page(url: &str) -> bool {
	let Some((kind, slug)) = url.strip_prefix('/').and_then(|path| path.split_once('/')) else {
		return false;
	};
	let Some((id, name)) = slug.split_once('-') else {
		return false;
	};
	TAG_KINDS.contains(&kind)
		&& !id.is_empty()
		&& id.bytes().all(|byte| byte.is_ascii_digit())
		&& !name.is_empty()
		&& name
			.chars()
			.all(|char| char.is_alphanumeric() || char == '-')
}
