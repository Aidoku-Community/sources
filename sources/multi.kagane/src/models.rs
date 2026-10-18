use aidoku::{
	Chapter, Manga, MangaWithChapter,
	alloc::{format, string::String, vec, vec::Vec},
	imports::std::parse_date,
};
use serde::Deserialize;
use serde_json::Value;

use crate::helpers::{api_base, base_url, source_title};

fn default_true() -> bool {
	true
}

// ── Search ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchResponse {
	#[serde(default)]
	pub content: Vec<SearchItem>,
	#[serde(default = "default_true")]
	pub last: bool,
}

#[derive(Deserialize)]
pub struct SearchItem {
	pub series_id: String,
	pub title: String,
	pub cover_image_id: Option<String>,
	pub source_id: Option<String>,
	#[serde(default)]
	pub latest_chapters: Vec<LatestChapter>,
}

#[derive(Deserialize)]
pub struct LatestChapter {
	pub book_id: String,
	pub title: Option<String>,
	pub chapter_no: Option<String>,
	pub volume_no: Option<String>,
	pub created_at: Option<String>,
}

impl SearchItem {
	/// `sources` is empty unless titles name their source.
	pub fn into_manga(self, sources: &[UploadSource]) -> Manga {
		Manga {
			title: source_title(&self.title, sources, self.source_id.as_deref()),
			cover: self
				.cover_image_id
				.map(|id| format!("{}/image/{id}", api_base())),
			url: Some(format!("{}/series/{}", base_url(), self.series_id)),
			key: self.series_id,
			..Default::default()
		}
	}

	/// The series with its newest chapter, or `None` when it has none yet.
	pub fn into_manga_with_chapter(mut self, sources: &[UploadSource]) -> Option<MangaWithChapter> {
		let book = self.latest_chapters.drain(..).next()?;
		Some(MangaWithChapter {
			chapter: Chapter {
				key: book.book_id,
				chapter_number: book.chapter_no.as_deref().and_then(|s| s.parse().ok()),
				volume_number: book.volume_no.as_deref().and_then(|s| s.parse().ok()),
				title: book.title.and_then(|t| {
					let t = t.trim();
					if t.is_empty() {
						None
					} else {
						Some(String::from(t))
					}
				}),
				date_uploaded: book.created_at.as_deref().and_then(|s| {
					let s = s.split_once('.').map_or(s, |(b, _)| b);
					parse_date(format!("{s}Z"), "yyyy-MM-dd'T'HH:mm:ss'Z'")
				}),
				..Default::default()
			},
			manga: self.into_manga(sources),
		})
	}
}

// ── Series detail ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SeriesDetail {
	pub title: String,
	pub description: Option<String>,
	pub upload_status: String,
	pub format: Option<String>,
	pub content_rating: Option<String>,
	pub age_rating: Option<String>,
	pub is_pornographic: Option<bool>,
	pub source_id: Option<String>,
	#[serde(default)]
	pub series_staff: Vec<StaffMember>,
	#[serde(default)]
	pub genres: Vec<GenreItem>,
	#[serde(default)]
	pub tags: Vec<TagItem>,
	#[serde(default)]
	pub series_covers: Vec<CoverItem>,
}

#[derive(Deserialize)]
pub struct StaffMember {
	pub name: String,
	pub role: String,
}

#[derive(Deserialize)]
pub struct GenreItem {
	pub genre_name: String,
	#[serde(default)]
	pub is_spoiler: bool,
}

#[derive(Deserialize)]
pub struct TagItem {
	pub tag_name: String,
	#[serde(default)]
	pub is_spoiler: bool,
}

#[derive(Deserialize)]
pub struct GroupItem {
	pub title: String,
}

#[derive(Deserialize)]
pub struct CoverItem {
	pub image_id: String,
}

#[derive(Deserialize)]
pub struct SourceList {
	#[serde(default)]
	pub sources: Vec<UploadSource>,
}

#[derive(Deserialize)]
pub struct TagEntry {
	pub id: String,
	pub tag_name: String,
}

#[derive(Deserialize)]
pub struct StaffResults {
	#[serde(default)]
	pub content: Vec<StaffResult>,
}

#[derive(Deserialize)]
pub struct StaffResult {
	pub staff_id: String,
	pub name: String,
}

#[derive(Deserialize)]
pub struct GroupResults {
	#[serde(default)]
	pub items: Vec<GroupResult>,
}

#[derive(Deserialize)]
pub struct GroupResult {
	pub group_id: String,
	pub title: String,
}

#[derive(Deserialize)]
pub struct UploadSource {
	pub source_id: String,
	pub title: String,
	pub source_type: Option<String>,
}

/// The chapter list: each book is a row of values in the order `fields` names, and its
/// groups are positions in `groups`.
#[derive(Deserialize)]
pub struct BookIndex {
	pub fields: Vec<String>,
	#[serde(default)]
	pub groups: Vec<GroupItem>,
	#[serde(default)]
	pub books: Vec<Vec<Value>>,
}

fn cell(row: &[Value], at: Option<usize>) -> Option<&Value> {
	row.get(at?)
}

impl BookIndex {
	/// Newest first by `sort_no`, the site's reading order, since chapter numbers can restart.
	/// `scanlator` names the series' source, which credits a chapter better than its groups.
	pub fn into_chapters(self, series_key: &str, scanlator: Option<&str>) -> Vec<Chapter> {
		let column = |name: &str| self.fields.iter().position(|field| field == name);
		let [id, title, sort, chapter, volume, released, group_ix] = [
			"book_id",
			"title",
			"sort_no",
			"chapter_no",
			"volume_no",
			"released_at",
			"group_ix",
		]
		.map(column);

		let mut books: Vec<(f64, Chapter)> = self
			.books
			.iter()
			.filter_map(|row| {
				let text = |at| cell(row, at).and_then(Value::as_str);
				let key = text(id)?;
				let scanlators: Vec<String> = match scanlator {
					Some(scanlator) => vec![scanlator.into()],
					None => cell(row, group_ix)
						.and_then(Value::as_array)
						.into_iter()
						.flatten()
						.filter_map(|at| self.groups.get(at.as_u64()? as usize))
						.map(|group| group.title.clone())
						.collect(),
				};
				let chapter = Chapter {
					key: key.into(),
					title: text(title)
						.map(str::trim)
						.filter(|title| !title.is_empty())
						.map(String::from),
					chapter_number: text(chapter).and_then(|number| number.parse().ok()),
					volume_number: text(volume).and_then(|number| number.parse().ok()),
					date_uploaded: cell(row, released).and_then(Value::as_i64),
					scanlators: (!scanlators.is_empty()).then_some(scanlators),
					url: Some(format!("{}/series/{series_key}/reader/{key}", base_url())),
					..Default::default()
				};
				let order = cell(row, sort).and_then(Value::as_f64).unwrap_or_default();
				Some((order, chapter))
			})
			.collect();
		books.sort_by(|a, b| b.0.total_cmp(&a.0));
		books.into_iter().map(|(_, chapter)| chapter).collect()
	}
}

// ── Page listing (DRM) ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct IntegrityResponse {
	pub token: String,
}

#[derive(Deserialize)]
pub struct ChallengeResponse {
	pub access_token: String,
	pub cache_url: String,
	pub manifest: Option<ManifestData>,
}

#[derive(Deserialize)]
pub struct ManifestData {
	#[serde(default)]
	pub pages: Vec<PageData>,
}

#[derive(Deserialize)]
pub struct PageData {
	pub page_no: i32,
	pub page_id: String,
	pub ext: Option<String>,
}
