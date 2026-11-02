use crate::{
	BASE_URL, CDN_URL,
	helpers::{key, markdown},
};
use aidoku::{
	AidokuError, Chapter, ContentRating, Link, LinkValue, Manga, MangaStatus, MangaWithChapter,
	Result, Viewer,
	alloc::{String, Vec, collections::BTreeMap, string::ToString},
	prelude::*,
};
use core::mem;
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

/// A series or novel as browse, the home blocks and the details pages list it.
#[derive(Deserialize)]
pub struct Entry {
	series_id: Option<u32>,
	novel_id: Option<u32>,
	pub title: String,
	cover: Option<String>,
	last_edit: Option<i64>,
	pub status: Option<String>,
	pub r#type: Option<String>,
	pub year: Option<i32>,
	#[serde(rename = "popularityRank")]
	pub popularity_rank: Option<u32>,
	description: Option<String>,
	// Browse calls the tags `categories`.
	#[serde(default, alias = "categories")]
	pub tags: Vec<String>,
	#[serde(default)]
	author: Vec<String>,
	#[serde(default)]
	artist: Vec<String>,
	#[serde(default)]
	chapters: Vec<ChapterItem>,
	#[serde(flatten)]
	pub unlock: Unlock,
}

/// How a novel releases its early-access chapters.
#[derive(Default, Deserialize)]
pub struct Unlock {
	pub unlock_policy: Option<String>,
	pub locked_chapter_count: Option<u32>,
	pub locked_chapter_percent: Option<u32>,
	pub locked_chapter_max: Option<u32>,
}

impl Entry {
	fn cover_url(&self) -> Option<String> {
		let (folder, id) = match self.novel_id {
			Some(id) => ("novels", id),
			None => ("series", self.series_id?),
		};
		Some(format!(
			"{CDN_URL}/uploads/images/{folder}/{id}/{}?{}",
			self.cover.as_deref()?,
			self.last_edit.unwrap_or_default()
		))
	}

	pub fn into_latest(mut self) -> Option<MangaWithChapter> {
		let chapter = mem::take(&mut self.chapters).into_iter().next()?;
		let manga = Manga::try_from(self).ok()?;
		let url = format!("{BASE_URL}{}", manga.key);
		Some(MangaWithChapter {
			chapter: chapter.into_chapter(&url, false),
			manga,
		})
	}

	pub fn into_details(self) -> Result<Manga> {
		let key = key(self.novel_id, self.series_id)?;
		let status = match self.status.as_deref().map(str::to_lowercase).as_deref() {
			Some("ongoing") => MangaStatus::Ongoing,
			Some("completed") => MangaStatus::Completed,
			Some("hiatus") => MangaStatus::Hiatus,
			Some("cancelled" | "dropped") => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		};
		let content_rating = if self
			.tags
			.iter()
			.any(|tag| matches!(tag.as_str(), "Ecchi" | "Ecchi comedy" | "Mature"))
		{
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		};
		let viewer = match self.r#type.as_deref() {
			Some("Manga") => Viewer::RightToLeft,
			Some("Novel" | "Web Novel") => Viewer::Unknown,
			_ => Viewer::Webtoon,
		};
		// A few credits arrive blank or padded.
		let names = |list: Vec<String>| {
			let list: Vec<String> = list
				.into_iter()
				.map(|name| name.trim().to_string())
				.filter(|name| !name.is_empty())
				.collect();
			(!list.is_empty()).then_some(list)
		};
		Ok(Manga {
			cover: self.cover_url(),
			url: Some(format!("{BASE_URL}{key}")),
			description: self
				.description
				.as_deref()
				.map(markdown)
				.filter(|text| !text.is_empty()),
			authors: names(self.author),
			artists: names(self.artist),
			tags: (!self.tags.is_empty()).then_some(self.tags),
			key,
			title: self.title,
			status,
			content_rating,
			viewer,
			..Default::default()
		})
	}
}

/// A card: key, title and cover only.
impl TryFrom<Entry> for Manga {
	type Error = AidokuError;

	fn try_from(entry: Entry) -> Result<Self> {
		Ok(Manga {
			key: key(entry.novel_id, entry.series_id)?,
			cover: entry.cover_url(),
			title: entry.title,
			..Default::default()
		})
	}
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub chapter: String,
	pub title: Option<String>,
	pub token: String,
	pub release_date: Option<i64>,
	pub price: Option<String>,
	pub public_release: Option<i64>,
}

impl ChapterItem {
	pub fn number(&self) -> Option<f32> {
		self.chapter.parse().ok()
	}

	pub fn into_chapter(self, manga_url: &str, locked: bool) -> Chapter {
		let key = format!("/{}", self.token);
		Chapter {
			url: Some(format!("{manga_url}{key}")),
			chapter_number: self.number(),
			title: self.title.filter(|title| !title.is_empty()),
			date_uploaded: self.release_date,
			locked,
			key,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct HomeProps {
	pub carousel: Vec<Slide>,
	#[serde(rename = "popularEntries")]
	pub popular: Blocks,
	#[serde(rename = "latestEntries")]
	pub latest: Blocks,
	pub novels: Blocks,
	#[serde(rename = "staffPicks")]
	pub staff_picks: Blocks,
	#[serde(rename = "latestNovels")]
	pub latest_novels: Blocks,
}

#[derive(Deserialize)]
pub struct Blocks {
	blocks: Vec<Block>,
}

#[derive(Deserialize)]
pub struct Block {
	pub title: String,
	pub series: Vec<Entry>,
}

impl Blocks {
	pub fn into_block(self) -> Option<Block> {
		self.blocks.into_iter().next()
	}
}

#[derive(Deserialize)]
pub struct Slide {
	series_id: Option<u32>,
	novel_id: Option<u32>,
	title: String,
	image: String,
}

impl Slide {
	pub fn into_link(self) -> Option<Link> {
		let key = key(self.novel_id, self.series_id).ok()?;
		Some(Link {
			title: self.title,
			image_url: Some(format!("{CDN_URL}/uploads/images/carousel/{}", self.image)),
			value: Some(LinkValue::Manga(Manga {
				key,
				..Default::default()
			})),
			..Default::default()
		})
	}
}

#[derive(Deserialize)]
pub struct BrowseProps {
	pub series: Vec<Entry>,
}

#[derive(Deserialize)]
pub struct SeriesProps {
	// A novel's page names its entry `novels`.
	#[serde(alias = "novels")]
	pub series: Entry,
	pub chapters: Vec<ChapterItem>,
}

#[derive(Deserialize)]
pub struct ChapterProps {
	pub chapter: ReaderChapter,
}

#[derive(Deserialize)]
pub struct ReaderChapter {
	pub edit_time: Option<i64>,
	#[serde(default)]
	pub images: BTreeMap<u32, Image>,
	pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct Image {
	pub name: String,
}
