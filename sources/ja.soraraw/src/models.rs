use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString},
	imports::std::parse_date_with_options,
	prelude::format,
};
use serde::Deserialize;

use crate::{DATE_FORMAT, THUMBNAIL_URL, chapter_key, chapter_url, manga_url, strip_html};

/// Wrapper of the "__NEXT_DATA__" blob every page embeds.
#[derive(Deserialize)]
pub struct NextData<T> {
	pub props: Props<T>,
}

#[derive(Deserialize)]
pub struct Props<T> {
	#[serde(rename = "pageProps")]
	pub page_props: T,
}

/// Page props of every page but the home page, which carries an extra list.
#[derive(Deserialize)]
pub struct DataProps<T> {
	pub data: T,
}

/// Page props of "/", the only page holding both the popular and the trending lists.
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

/// Data of the paginated listing pages ("/", "/newest" and "/genre/{slug}").
#[derive(Deserialize)]
pub struct ListData {
	/// Popular entries, only filled in on the home page.
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

/// A manga as it appears in a listing or in the search results.
///
/// The listings don't all carry the same fields, so everything but the two the app needs to show
/// a cover is optional here.
#[derive(Deserialize)]
pub struct MangaEntry {
	pub name: String,
	pub slug: String,
	pub author: Option<String>,
	/// Cover file name on the thumbnail host.
	pub image: Option<String>,
	/// Full cover url, which only the home and genre listings provide.
	pub thumbnail: Option<String>,
	/// Publishing status, either "incomplete" or "complete".
	#[serde(rename = "type")]
	pub kind: Option<String>,
	/// Reading direction, either "horizontal" or "vertical".
	pub mode: Option<String>,
	pub is_adult: Option<String>,
}

impl From<MangaEntry> for Manga {
	fn from(value: MangaEntry) -> Self {
		Manga {
			cover: cover(value.thumbnail, value.image.as_deref()),
			title: value.name.trim().into(),
			authors: authors(value.author.as_deref()),
			url: Some(manga_url(&value.slug)),
			key: value.slug,
			status: status(value.kind.as_deref()),
			content_rating: content_rating(value.is_adult.as_deref()),
			viewer: viewer(value.mode.as_deref()),
			..Default::default()
		}
	}
}

/// Data of "/manga/{slug}".
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
	/// Always null in practice; the synopsis lives in "content" instead.
	pub description: Option<String>,
	/// Editor.js document holding the synopsis.
	pub content: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub mode: Option<String>,
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

	pub fn authors(&self) -> Option<Vec<String>> {
		authors(self.author.as_deref())
	}

	/// Reads the synopsis out of the Editor.js document the site stores it as, falling back to
	/// the plain field for the entries that happen to fill it in.
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
}

impl Genre {
	pub fn into_tag(self) -> Option<String> {
		let tag = String::from(self.name.trim());
		(!tag.is_empty()).then_some(tag)
	}
}

/// A block document as produced by Editor.js, which the site stores synopses in.
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
	/// Holds inline markup, so it can't be used as it is.
	pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterEntry {
	pub id: i64,
	/// Chapter number, given as a number for most entries and as a string for a few.
	pub name: Option<Number>,
	pub title: Option<String>,
	/// Slug of the chapter, prefixed with the slug of the manga it belongs to.
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

/// Data of "/manga/{slug}/{chapter}", read only to resolve deep links.
#[derive(Deserialize)]
pub struct ChapterData {
	pub chapter: Option<ChapterDetails>,
}

#[derive(Deserialize)]
pub struct ChapterDetails {
	pub id: i64,
	pub manga_id: i64,
}

/// Response of the image endpoint, which hands out the page list as an obfuscated payload.
#[derive(Deserialize)]
pub struct ImagePayload {
	pub d: String,
}

/// A page as listed in the decoded payload.
///
/// Every entry also carries a "b" and a "d" field holding the encrypted name of the file on each
/// of the two image servers. Those are left alone: the site encrypts them with a key its scripts
/// derive at runtime, and the name can be rebuilt from the two fields that aren't encrypted.
#[derive(Deserialize)]
pub struct PageImage {
	pub id: i64,
	pub order: Number,
}

impl PageImage {
	/// File name of the page, without the extension.
	pub fn file_name(&self) -> Option<String> {
		let order = self.order.as_f32()? as i32;
		(order > 0).then(|| format!("{order:03}_{}", self.id))
	}

	pub fn order(&self) -> i32 {
		self.order.as_f32().unwrap_or_default() as i32
	}
}

/// A value the site gives as either a number or a string, depending on the entry.
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

/// An entry of "/genres.json", used to build the genre filter.
#[derive(Deserialize)]
pub struct GenreEntry {
	pub name: String,
	pub slug: String,
}

/// Listings hand out either a full cover url or just the file name on the thumbnail host.
fn cover(thumbnail: Option<String>, image: Option<&str>) -> Option<String> {
	thumbnail
		.filter(|thumbnail| !thumbnail.is_empty())
		.or_else(|| {
			image
				.filter(|image| !image.is_empty())
				.map(|image| format!("{THUMBNAIL_URL}/{image}"))
		})
}

/// Authors come as a single comma separated field.
fn authors(author: Option<&str>) -> Option<Vec<String>> {
	let authors = author?
		.split(',')
		.map(|author| String::from(author.trim()))
		.filter(|author| !author.is_empty())
		.collect::<Vec<String>>();
	(!authors.is_empty()).then_some(authors)
}

pub fn status(kind: Option<&str>) -> MangaStatus {
	match kind {
		Some("complete") => MangaStatus::Completed,
		Some("incomplete") => MangaStatus::Ongoing,
		_ => MangaStatus::Unknown,
	}
}

/// The site marks webtoons by the reading direction it renders them with.
pub fn viewer(mode: Option<&str>) -> Viewer {
	match mode {
		Some("vertical") => Viewer::Webtoon,
		Some("horizontal") => Viewer::RightToLeft,
		_ => Viewer::Unknown,
	}
}

/// Every entry carries the flag the site sorts adult content by, which is what this follows.
///
/// Deriving anything further from the genres of a series was tried and dropped: genre names are
/// not unique (41 of the 1834 the site lists are used by more than one genre), and the ones that
/// read as suggestive are already flagged as adult by the site itself, so a name based guess
/// disagreed with the site more often than it added anything.
pub fn content_rating(is_adult: Option<&str>) -> ContentRating {
	match is_adult {
		Some("yes") => ContentRating::NSFW,
		Some("no") => ContentRating::Safe,
		_ => ContentRating::Unknown,
	}
}

/// Decodes the base64 payload the image endpoint returns, accepting both the standard and the url
/// safe alphabet and treating padding as optional, the same way the site's own decoder does.
pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
	let mut output = Vec::with_capacity(input.len() / 4 * 3);
	let mut buffer = 0u32;
	let mut bits = 0u32;

	for byte in input.bytes() {
		let value = match byte {
			b'A'..=b'Z' => byte - b'A',
			b'a'..=b'z' => byte - b'a' + 26,
			b'0'..=b'9' => byte - b'0' + 52,
			b'+' | b'-' => 62,
			b'/' | b'_' => 63,
			b'=' => break,
			b' ' | b'\t' | b'\r' | b'\n' => continue,
			_ => return None,
		};
		buffer = (buffer << 6) | u32::from(value);
		bits += 6;
		if bits >= 8 {
			bits -= 8;
			output.push((buffer >> bits) as u8);
		}
	}

	Some(output)
}

/// Turns the payload of the image endpoint back into the json it was built from. The site xors it
/// with a fixed key, which is the only thing standing between the endpoint and a page list.
pub fn deobfuscate(payload: &str, key: &[u8]) -> Option<String> {
	if key.is_empty() {
		return None;
	}

	let mut bytes = decode_base64(payload)?;
	for (index, byte) in bytes.iter_mut().enumerate() {
		*byte ^= key[index % key.len()];
	}

	let json = String::from_utf8(bytes).ok()?;
	// the payloads carry a byte order mark and trailing padding that json won't accept
	let json = json
		.trim_start_matches('\u{feff}')
		.trim_matches(|char| char == '\0' || char::is_whitespace(char));
	(!json.is_empty()).then(|| json.to_string())
}
