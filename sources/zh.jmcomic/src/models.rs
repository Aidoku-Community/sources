use crate::WEB_URL;
use aidoku::{
	Chapter, ContentRating, Listing, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	prelude::format,
};
use core::cmp::Ordering;
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use serde_json::Value;

fn flex_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
	Value::deserialize(d).map(|v| match v {
		Value::String(s) => s,
		Value::Number(n) => n.to_string(),
		_ => String::new(),
	})
}

fn flex_i32<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
	flex_string(d).and_then(|s| s.parse().map_err(Error::custom))
}

fn flex_f32<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
	flex_string(d).and_then(|s| s.parse().map_err(Error::custom))
}

fn flex_optional<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
	Option::<Value>::deserialize(d).map(|opt| match opt {
		Some(Value::String(s)) => s,
		Some(Value::Number(n)) => n.to_string(),
		_ => String::new(),
	})
}

#[derive(Deserialize)]
pub struct ApiOuter {
	pub data: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AuthData {
	#[serde(alias = "jwttoken")]
	pub jwt_token: String,
}

impl AuthData {
	pub fn is_valid(&self) -> bool {
		!self.jwt_token.trim().is_empty()
	}
}

pub struct BlockState {
	keywords: Vec<String>,
	ids: Vec<String>,
}

impl BlockState {
	pub fn new(entries: Vec<String>) -> Self {
		let mut keywords = Vec::new();
		let mut ids = Vec::new();
		for entry in entries {
			if entry.chars().all(|c| c.is_ascii_digit()) {
				ids.push(entry);
			} else {
				keywords.push(entry);
			}
		}
		Self { keywords, ids }
	}

	pub fn is_empty(&self) -> bool {
		self.keywords.is_empty() && self.ids.is_empty()
	}

	pub fn is_blocked<'a>(&self, id: &str, fields: impl IntoIterator<Item = &'a str>) -> bool {
		if self.ids.iter().any(|i| i == id) {
			return true;
		}
		if self.keywords.is_empty() {
			return false;
		}
		for field in fields {
			let lc = field.trim().to_lowercase();
			if self.keywords.iter().any(|kw| lc.contains(kw.as_str())) {
				return true;
			}
		}
		false
	}
}

#[derive(Default, Deserialize)]
pub struct CategoryRef {
	pub title: Option<String>,
}

#[derive(Deserialize)]
pub struct ComicItem {
	#[serde(deserialize_with = "flex_string")]
	pub id: String,
	pub name: Option<String>,
	pub author: Option<String>,
	#[serde(default)]
	pub image: Option<String>,
	pub description: Option<String>,
	#[serde(default)]
	pub category: Option<CategoryRef>,
	#[serde(default)]
	pub category_sub: Option<CategoryRef>,
}

#[derive(Deserialize)]
pub struct SearchResp {
	#[serde(deserialize_with = "flex_i32")]
	pub total: i32,
	#[serde(default, alias = "list")]
	pub content: Vec<ComicItem>,
}

pub fn cover_url(cdn_base: &str, id: &str) -> String {
	format!("{cdn_base}/media/albums/{id}_3x4.jpg")
}

fn resolve_cover(cdn_base: &str, id: &str, image: Option<&str>) -> String {
	image
		.filter(|v| !v.is_empty())
		.map(Into::into)
		.unwrap_or_else(|| cover_url(cdn_base, id))
}

fn ascii_ratio(s: &str) -> f32 {
	if s.is_empty() {
		return 0.0;
	}
	let total = s.chars().count();
	let ascii = s.chars().filter(|c| c.is_ascii()).count();
	ascii as f32 / total as f32
}

fn has_japanese(s: &str) -> bool {
	s.chars()
		.any(|c| matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}'))
}

impl SearchResp {
	pub fn into_manga_list(self, cdn_base: &str, ctx: &BlockState) -> Vec<Manga> {
		self.content
			.into_iter()
			.filter(|item| !item.is_blocked(ctx))
			.map(|item| item.into_manga(cdn_base))
			.collect()
	}
}

impl ComicItem {
	fn block_fields(&self) -> impl Iterator<Item = &str> {
		[
			self.name.as_deref(),
			self.author.as_deref(),
			self.description.as_deref(),
		]
		.into_iter()
		.flatten()
		.chain(self.category.as_ref().and_then(|c| c.title.as_deref()))
		.chain(self.category_sub.as_ref().and_then(|c| c.title.as_deref()))
	}

	pub fn is_blocked(&self, ctx: &BlockState) -> bool {
		ctx.is_blocked(&self.id, self.block_fields())
	}

	pub fn into_manga(self, cdn_base: &str) -> Manga {
		let cover = Some(resolve_cover(cdn_base, &self.id, self.image.as_deref()));
		let authors = self
			.author
			.filter(|author| !author.trim().is_empty())
			.map(|author| vec![author]);
		Manga {
			key: self.id,
			title: self.name.unwrap_or_default(),
			cover,
			authors,
			content_rating: ContentRating::NSFW,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct AlbumResp {
	pub name: Option<String>,
	pub author: Option<Vec<String>>,
	#[serde(default)]
	pub addtime: Option<String>,
	pub description: Option<String>,
	pub tags: Option<Vec<String>>,
	#[serde(default)]
	pub works: Option<Vec<String>>,
	pub series: Option<Vec<SeriesItem>>,
}

#[derive(Deserialize)]
pub struct SeriesItem {
	#[serde(deserialize_with = "flex_string")]
	pub id: String,
	pub name: Option<String>,
	#[serde(default, deserialize_with = "flex_f32")]
	pub sort: f32,
}

#[derive(Deserialize)]
pub struct ChapterResp {
	#[serde(deserialize_with = "flex_string")]
	pub id: String,
	#[serde(default, deserialize_with = "flex_optional")]
	pub series_id: String,
	pub images: Vec<String>,
}

impl ChapterResp {
	pub fn series_key<'a>(&'a self, fallback: &'a str) -> &'a str {
		if self.series_id.is_empty() || self.series_id == "0" {
			fallback
		} else {
			self.series_id.as_str()
		}
	}

	pub fn episode_id<'a>(&'a self, fallback: &'a str) -> &'a str {
		if self.id.is_empty() {
			fallback
		} else {
			self.id.as_str()
		}
	}
}

impl AlbumResp {
	pub fn is_missing(&self) -> bool {
		self.name.as_deref().map(str::trim).unwrap_or("").is_empty()
	}

	fn labels(&self) -> impl Iterator<Item = &str> {
		self.tags
			.iter()
			.flatten()
			.chain(self.works.iter().flatten())
			.map(String::as_str)
	}

	fn block_fields(&self) -> impl Iterator<Item = &str> {
		[self.name.as_deref(), self.description.as_deref()]
			.into_iter()
			.flatten()
			.chain(self.author.iter().flatten().map(String::as_str))
			.chain(self.labels())
			.chain(
				self.series
					.iter()
					.flatten()
					.filter_map(|s| s.name.as_deref()),
			)
	}

	pub fn is_blocked(&self, key: &str, ctx: &BlockState) -> bool {
		ctx.is_blocked(key, self.block_fields())
	}

	fn content_rating(&self) -> ContentRating {
		let mut rating = ContentRating::NSFW;
		for label in self.labels() {
			if label == "非H" {
				return ContentRating::Safe;
			}
			if label == "青年漫" {
				rating = ContentRating::Suggestive;
			}
		}
		rating
	}

	pub fn into_manga(self, key: &str, cdn_base: &str) -> Manga {
		let status = self.status();
		let content_rating = self.content_rating();
		let viewer = self.viewer().unwrap_or_default();
		Manga {
			key: key.into(),
			title: self.name.unwrap_or_default(),
			cover: Some(cover_url(cdn_base, key)),
			authors: self.author,
			description: self.description.filter(|s| !s.trim().is_empty()),
			tags: self.tags.map(|tags| {
				tags.into_iter()
					.filter_map(|tag| {
						let t = tag.trim();
						(!t.is_empty()).then(|| t.into())
					})
					.collect()
			}),
			url: Some(format!("{WEB_URL}/album/{key}")),
			status,
			content_rating,
			viewer,
			..Default::default()
		}
	}

	pub fn to_chapters(&self, manga_key: &str) -> Vec<Chapter> {
		let series = self.series.as_deref().unwrap_or(&[]);
		let album_time: Option<i64> = self.addtime.as_deref().and_then(|s| s.parse().ok());

		if series.is_empty() {
			return vec![Chapter {
				key: manga_key.into(),
				chapter_number: Some(1.0),
				date_uploaded: album_time,
				url: Some(format!("{WEB_URL}/photo/{manga_key}")),
				..Default::default()
			}];
		}

		let mut indexed: Vec<_> = series.iter().enumerate().collect();
		indexed.sort_by(
			|(li, l), (ri, r)| match chapter_sort_order(l, *li, r, *ri) {
				Ordering::Equal => l.id.cmp(&r.id),
				other => other,
			},
		);

		let last_idx = indexed.len().saturating_sub(1);
		let mut chapters: Vec<_> = indexed
			.into_iter()
			.enumerate()
			.map(|(pos, (_, ch))| {
				let num = normalized_chapter_number(ch.sort, pos);
				Chapter {
					key: ch.id.clone(),
					title: Some(ch.display_title(pos + 1)),
					chapter_number: Some(num),
					date_uploaded: if pos == last_idx { album_time } else { None },
					url: Some(format!("{WEB_URL}/photo/{}", ch.id)),
					..Default::default()
				}
			})
			.collect();
		chapters.reverse();
		chapters
	}

	fn viewer(&self) -> Option<Viewer> {
		let mut has_english = false;
		for label in self.labels() {
			if matches!(label, "条漫" | "韩漫" | "一般向韩漫" | "韩国")
				|| label.eq_ignore_ascii_case("webtoon")
			{
				return Some(Viewer::Webtoon);
			}
			if label == "美漫" {
				return Some(Viewer::LeftToRight);
			}
			has_english |= label == "英文";
		}

		let name = self.name.as_deref().unwrap_or("");
		let desc = self.description.as_deref().unwrap_or("");

		if has_japanese(name) || has_japanese(desc) {
			return None;
		}

		if ascii_ratio(name) >= 0.45 {
			return Some(Viewer::LeftToRight);
		}
		if has_english && ascii_ratio(desc) >= 0.45 {
			return Some(Viewer::LeftToRight);
		}
		None
	}

	fn status(&self) -> MangaStatus {
		let mut ongoing = false;
		for tag in self.tags.iter().flatten().map(String::as_str) {
			if matches!(tag, "完结" | "已完结") {
				return MangaStatus::Completed;
			}
			ongoing |= matches!(tag, "连载中" | "连载");
		}
		if ongoing {
			MangaStatus::Ongoing
		} else {
			MangaStatus::Unknown
		}
	}
}

impl SeriesItem {
	fn display_title(&self, fallback_index: usize) -> String {
		self.name
			.as_deref()
			.map(str::trim)
			.filter(|s| !s.is_empty())
			.map(Into::into)
			.unwrap_or_else(|| {
				let num = normalized_chapter_number(self.sort, fallback_index.saturating_sub(1));
				format!("第{}話", format_chapter_number(num))
			})
	}
}

fn chapter_sort_order(l: &SeriesItem, li: usize, r: &SeriesItem, ri: usize) -> Ordering {
	match (l.sort > 0.0, r.sort > 0.0) {
		(true, true) => l.sort.total_cmp(&r.sort),
		(false, false) => li.cmp(&ri),
		(true, false) => Ordering::Less,
		(false, true) => Ordering::Greater,
	}
}

fn normalized_chapter_number(sort: f32, index: usize) -> f32 {
	if sort > 0.0 { sort } else { (index + 1) as f32 }
}

fn format_chapter_number(v: f32) -> String {
	let w = v as i32;
	if w as f32 == v {
		format!("{w}")
	} else {
		format!("{v}")
			.trim_end_matches('0')
			.trim_end_matches('.')
			.into()
	}
}

#[derive(Deserialize, Default)]
pub struct DomainRefreshResp {
	#[serde(rename = "Server", default)]
	pub server: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct SettingData {
	#[serde(default)]
	pub img_host: Option<String>,
}

#[derive(Deserialize)]
pub struct PromoteGroup {
	#[serde(deserialize_with = "flex_string")]
	pub id: String,
	pub title: Option<String>,
	#[serde(rename = "type")]
	pub group_type: Option<String>,
	pub slug: Option<String>,
	pub content: Vec<PromoteItem>,
}

#[derive(Deserialize)]
pub struct PromoteItem {
	#[serde(deserialize_with = "flex_string")]
	pub id: String,
	pub name: Option<String>,
	pub author: Option<String>,
	#[serde(default)]
	pub category: Option<CategoryRef>,
	#[serde(default)]
	pub category_sub: Option<CategoryRef>,
	pub image: Option<String>,
}

impl PromoteGroup {
	const PAGED_IDS: [&str; 2] = ["29", "30"];

	pub fn is_visible(&self) -> bool {
		!self.content.is_empty()
			&& !matches!(
				self.group_type.as_deref().unwrap_or(""),
				"library" | "novels"
			) && self.slug.as_deref() != Some("another")
	}

	pub fn is_large_listing_id(group_id: &str) -> bool {
		Self::PAGED_IDS.contains(&group_id)
	}

	pub fn listing(&self) -> Option<Listing> {
		let slug = self.slug.as_deref().unwrap_or("");
		let gtype = self.group_type.as_deref().unwrap_or("");
		let id = match (gtype, slug) {
			(_, "latest" | "new" | "recent") => return None,
			(_, "most_viewed" | "popular" | "mv") => "o:mv".into(),
			("category_id", "") => return None,
			("category_id", c) => format!("cat:{c}"),
			("not_in_category_id", _) => {
				let q = self
					.slug
					.as_deref()
					.filter(|v| !v.is_empty())
					.or(self.title.as_deref())
					.filter(|v| !v.is_empty())?;
				format!("q:{q}")
			}
			("promote", _) => format!("promo:{}", self.id),
			_ => return None,
		};
		Some(Listing {
			id,
			name: self
				.title
				.as_deref()
				.filter(|title| !title.is_empty())
				.unwrap_or(slug)
				.into(),
			..Default::default()
		})
	}

	pub fn into_manga_list(self, cdn_base: &str, ctx: &BlockState) -> Vec<Manga> {
		self.content
			.into_iter()
			.map(ComicItem::from)
			.filter(|item| !item.is_blocked(ctx))
			.map(|item| item.into_manga(cdn_base))
			.collect()
	}
}

impl From<PromoteItem> for ComicItem {
	fn from(value: PromoteItem) -> Self {
		Self {
			id: value.id,
			name: value.name,
			author: value.author,
			image: value.image,
			description: None,
			category: value.category,
			category_sub: value.category_sub,
		}
	}
}
