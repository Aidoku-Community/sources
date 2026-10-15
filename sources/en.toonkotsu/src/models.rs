use crate::{
	api::BASE_URL,
	helpers::{clean_text, cover_url, is_unlisted_tag, manga_url, rating_id, split_label},
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{
		String, Vec,
		collections::{BTreeMap, BTreeSet},
		format,
		string::ToString,
		vec,
	},
};
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Series {
	/// The id every url and request uses.
	pub hid: Option<String>,
	pub slug: Option<String>,
	pub title: String,
	pub description: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub tags: Option<Vec<String>>,
	pub spoiler_tags: Option<Vec<String>>,
	pub status: Option<String>,
	pub cover: Option<String>,
	pub authors: Option<Vec<String>>,
	pub adult: Option<bool>,
}

impl Series {
	fn content_rating(&self) -> ContentRating {
		match rating_id(self) {
			"safe" => ContentRating::Safe,
			"suggestive" => ContentRating::Suggestive,
			_ => ContentRating::NSFW,
		}
	}

	/// What a card shows; the rest arrives with the details.
	pub fn into_manga(self) -> Option<Manga> {
		let content_rating = self.content_rating();
		let key = self.hid.filter(|hid| !hid.is_empty())?;
		Some(Manga {
			url: Some(manga_url(&key, self.slug.as_deref())),
			cover: cover_url(self.cover.as_deref(), false),
			title: clean_text(&self.title),
			content_rating,
			key,
			..Default::default()
		})
	}

	pub fn into_full_manga(self) -> Option<Manga> {
		let content_rating = self.content_rating();
		let key = self.hid.filter(|hid| !hid.is_empty())?;
		let spoilers = self.spoiler_tags.unwrap_or_default();
		let tags: Vec<String> = self
			.tags
			.unwrap_or_default()
			.into_iter()
			.filter(|tag| !is_unlisted_tag(tag) && !spoilers.contains(tag))
			.collect();
		let authors: Vec<String> = self
			.authors
			.unwrap_or_default()
			.iter()
			.map(|author| clean_text(author))
			.filter(|author| !author.is_empty())
			.collect();
		Some(Manga {
			url: Some(manga_url(&key, self.slug.as_deref())),
			cover: cover_url(self.cover.as_deref(), true),
			title: clean_text(&self.title),
			authors: (!authors.is_empty()).then_some(authors),
			description: self
				.description
				.as_deref()
				.map(clean_text)
				.filter(|text| !text.is_empty()),
			tags: (!tags.is_empty()).then_some(tags),
			status: match self.status.as_deref() {
				Some("Ongoing") => MangaStatus::Ongoing,
				Some("Complete") => MangaStatus::Completed,
				Some("Hiatus") => MangaStatus::Hiatus,
				Some("Canceled") => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			content_rating,
			// Korean and Chinese comics read as webtoons, which is how the site shows them.
			viewer: match self.kind.as_deref() {
				Some("Manhwa") | Some("Manhua") => Viewer::Webtoon,
				Some("Manga") => Viewer::RightToLeft,
				_ => Viewer::Unknown,
			},
			key,
			..Default::default()
		})
	}
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Group {
	pub name: Option<String>,
}

/// One upload of a chapter. A numbered chapter carries its alternates in `alts`.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Release {
	pub id: i64,
	pub label: Option<String>,
	pub at: Option<i64>,
	pub group: Option<Group>,
	/// Page width in pixels and download size in megabytes, which rank a group's uploads.
	pub w: Option<i64>,
	pub mb: Option<f64>,
}

/// The site numbers a volume either way round.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Volume {
	Number(f64),
	Text(String),
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ChapterEntry {
	#[serde(flatten)]
	pub release: Release,
	pub n: Option<f64>,
	pub vol: Option<Volume>,
	pub alts: Option<Vec<Release>>,
}

impl Release {
	pub fn into_chapter(self, number: Option<f64>, volume: Option<&Volume>) -> Chapter {
		let (title, bracketed) = self
			.label
			.as_deref()
			.map(|label| split_label(label, number))
			.unwrap_or_default();
		let scanlator = self
			.group
			.and_then(|group| group.name)
			.map(|name| clean_text(&name))
			.filter(|name| !name.is_empty())
			.or(bracketed);
		Chapter {
			url: Some(format!("{BASE_URL}/chapter/{}", self.id)),
			key: self.id.to_string(),
			// A label that only names the group is no title.
			title: title.filter(|title| {
				scanlator
					.as_deref()
					.is_none_or(|scanlator| !title.eq_ignore_ascii_case(scanlator))
			}),
			chapter_number: number.map(|number| number as f32),
			volume_number: match volume {
				Some(Volume::Number(number)) => Some(*number as f32),
				Some(Volume::Text(text)) => text.trim().parse().ok(),
				None => None,
			},
			// The catalogue times its uploads in milliseconds.
			date_uploaded: self.at.map(|at| at / 1000),
			scanlators: scanlator.map(|name| vec![name]),
			language: Some("en".into()),
			..Default::default()
		}
	}
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SeriesList {
	pub items: Option<Vec<Series>>,
	pub has_next_page: bool,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct LatestItem {
	pub chapter: ChapterEntry,
	pub series: Series,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct LatestList {
	pub items: Option<Vec<LatestItem>>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SeriesDetail {
	pub series: Series,
	pub chapters: Option<Vec<ChapterEntry>>,
}

/// The download as the site quotes it: whole megabytes from a hundred up, one decimal below.
fn size_label(mb: f64) -> Option<String> {
	if mb <= 0.0 {
		return None;
	}
	let size = if mb >= 100.0 {
		format!("{mb:.0}")
	} else {
		format!("{mb:.1}")
	};
	Some(format!("{} MB", size.strip_suffix(".0").unwrap_or(&size)))
}

/// One row per group for each chapter, keeping a group's widest, then largest, upload.
/// `main_only` keeps only the upload the site lists first; `show_size` keeps each size, named
/// in the title.
pub fn chapter_list(entries: Vec<ChapterEntry>, main_only: bool, show_size: bool) -> Vec<Chapter> {
	// A release can also be listed under more than one chapter.
	let mut ids: BTreeSet<i64> = BTreeSet::new();
	let mut slots: BTreeMap<String, usize> = BTreeMap::new();
	let mut kept: Vec<((i64, f64), Chapter)> = Vec::new();
	for ChapterEntry {
		release,
		n,
		vol,
		alts,
	} in entries
	{
		let alts = if main_only {
			Vec::new()
		} else {
			alts.unwrap_or_default()
		};
		for release in core::iter::once(release).chain(alts) {
			if !ids.insert(release.id) {
				continue;
			}
			let quality = (
				release.w.unwrap_or_default(),
				release.mb.unwrap_or_default(),
			);
			let size = show_size.then(|| size_label(quality.1)).flatten();
			let mut chapter = release.into_chapter(n, vol.as_ref());
			let group = chapter
				.scanlators
				.as_ref()
				.and_then(|names| names.first())
				.map(|name| name.to_lowercase());
			// Unnumbered releases are told apart by their titles.
			let title = chapter.chapter_number.is_none().then_some(&chapter.title);
			let slot = format!(
				"{:?} {:?} {group:?} {title:?} {size:?}",
				chapter.volume_number, chapter.chapter_number
			);
			if let Some(size) = size {
				chapter.title = Some(match chapter.title {
					Some(title) => format!("{title} ({size})"),
					None => size,
				});
			}
			match slots.get(&slot) {
				Some(&index) => {
					if quality > kept[index].0 {
						kept[index] = (quality, chapter);
					}
				}
				None => {
					slots.insert(slot, kept.len());
					kept.push((quality, chapter));
				}
			}
		}
	}
	let mut chapters: Vec<Chapter> = kept.into_iter().map(|(_, chapter)| chapter).collect();
	// Official releases restart their numbering each volume, so the list runs by volume, then
	// number. Chapters not yet in a volume are the newest, and a whole volume heads its own.
	let rank = |chapter: &Chapter| {
		(
			chapter.volume_number.unwrap_or(f32::INFINITY),
			chapter.chapter_number.unwrap_or(f32::INFINITY),
		)
	};
	chapters.sort_by(|a, b| {
		let (a, b) = (rank(a), rank(b));
		b.0.total_cmp(&a.0).then(b.1.total_cmp(&a.1))
	});
	chapters
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Facet {
	pub name: String,
	pub category: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Facets {
	pub genres: Option<Vec<Facet>>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderKey {
	pub key: String,
	/// The site's own clock, which signing uses so a wrong device clock cannot reject it.
	pub now: i64,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderPage {
	pub src: String,
	pub k: Option<String>,
	pub n: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderChapter {
	pub pages: Option<Vec<ReaderPage>>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderResponse {
	/// Absent or empty means the page keys are used as served.
	pub wrap: Option<String>,
	pub chapter: ReaderChapter,
}

/// The body of a refused read: a human check, or a wait the site names in seconds.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderLimit {
	pub error: Option<String>,
	pub code: Option<String>,
	pub reason: Option<String>,
	pub transient: Option<bool>,
	pub resign: Option<bool>,
	pub retry_after_sec: Option<i64>,
}
