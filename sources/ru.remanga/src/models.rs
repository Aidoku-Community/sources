use crate::settings::{SITE_URL, english_titles, media_url, show_paid_info};
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::{
	collections::BTreeMap,
	format,
	string::{String, ToString},
	vec::Vec,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CatalogResponse {
	pub next: Option<NextField>,
	pub results: Option<Vec<TitleCard>>,
}

#[derive(Deserialize)]
pub struct SearchResponse {
	pub results: Option<Vec<TitleCard>>,
	pub meta: Option<SearchMeta>,
}

#[derive(Deserialize)]
pub struct SearchMeta {
	pub page: Option<i32>,
	pub total_pages: Option<i32>,
}

#[derive(Deserialize)]
pub struct TitleCard {
	pub main_name: Option<String>,
	pub secondary_name: Option<String>,
	pub dir: Option<String>,
	pub cover: Option<Cover>,
	pub img: Option<Cover>,
	#[serde(rename = "type")]
	pub title_type: Option<TypeField>,
	pub status: Option<NamedId>,
	pub genres: Option<Vec<NamedId>>,
	pub is_erotic: Option<bool>,
	pub is_yaoi: Option<bool>,
	pub age_limit: Option<AgeLimit>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum TypeField {
	Object(NamedId),
	String(String),
}

#[derive(Deserialize, Clone)]
pub struct NamedId {
	pub id: Option<i32>,
	pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct AgeLimit {
	pub id: Option<i32>,
	pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct Cover {
	pub high: Option<String>,
	pub mid: Option<String>,
	pub low: Option<String>,
}

impl Cover {
	pub fn best(&self) -> Option<String> {
		self.high
			.as_ref()
			.or(self.mid.as_ref())
			.or(self.low.as_ref())
			.map(|p| media_url(p))
	}
}

#[derive(Deserialize)]
pub struct TitleDetail {
	pub main_name: Option<String>,
	pub secondary_name: Option<String>,
	pub dir: Option<String>,
	pub description: Option<String>,
	pub cover: Option<Cover>,
	#[serde(rename = "type")]
	pub title_type: Option<NamedId>,
	pub status: Option<NamedId>,
	pub age_limit: Option<AgeLimit>,
	pub genres: Option<Vec<NamedId>>,
	pub categories: Option<Vec<NamedId>>,
	/// API v1: object map; API v2: often `[]`.
	#[serde(default)]
	pub creators: Option<CreatorsField>,
	pub branches: Option<Vec<Branch>>,
	pub is_erotic: Option<bool>,
	pub is_yaoi: Option<bool>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CreatorsField {
	Map(BTreeMap<String, Vec<Creator>>),
	List(Vec<Creator>),
}

impl CreatorsField {
	pub fn into_author_names(self) -> Vec<String> {
		let mut authors = Vec::new();
		match self {
			Self::Map(map) => {
				for list in map.into_values() {
					for creator in list {
						push_unique_name(&mut authors, creator.name);
					}
				}
			}
			Self::List(list) => {
				for creator in list {
					push_unique_name(&mut authors, creator.name);
				}
			}
		}
		authors
	}
}

fn push_unique_name(authors: &mut Vec<String>, name: Option<String>) {
	if let Some(name) = name.filter(|s| !s.is_empty())
		&& !authors.iter().any(|a| a == &name)
	{
		authors.push(name);
	}
}

#[derive(Deserialize)]
pub struct Creator {
	pub name: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct Branch {
	pub id: i64,
	pub count_chapters: Option<i32>,
	pub publishers: Option<Vec<NamedId>>,
}

#[derive(Deserialize)]
pub struct ChaptersResponse {
	/// Catalog uses a URL string; chapters list uses a page number.
	pub next: Option<NextField>,
	pub results: Option<Vec<ApiChapter>>,
}

/// Pagination cursor that may be a URL, page number, or bool.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum NextField {
	Url(String),
	Page(i64),
	Flag(bool),
}

impl NextField {
	pub fn has_more(&self) -> bool {
		match self {
			Self::Url(url) => !url.is_empty(),
			Self::Page(page) => *page > 0,
			Self::Flag(flag) => *flag,
		}
	}
}

/// Lightweight title payload used only to discover translation branches.
#[derive(Deserialize)]
pub struct TitleBranches {
	pub branches: Option<Vec<Branch>>,
}

#[derive(Deserialize)]
pub struct ApiChapter {
	pub id: i64,
	pub index: Option<i32>,
	pub tome: Option<i32>,
	pub chapter: Option<FlexString>,
	pub name: Option<String>,
	pub upload_date: Option<String>,
	pub pub_date: Option<String>,
	pub is_paid: Option<bool>,
	pub is_bought: Option<bool>,
	pub price: Option<FlexString>,
	pub publishers: Option<Vec<NamedId>>,
}

/// String or number from JSON.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum FlexString {
	Text(String),
	Int(i64),
	Float(f64),
}

impl FlexString {
	pub fn as_string(&self) -> String {
		match self {
			Self::Text(s) => s.clone(),
			Self::Int(v) => format!("{v}"),
			Self::Float(v) => format!("{v}"),
		}
	}

	pub fn as_f32(&self) -> Option<f32> {
		match self {
			Self::Int(v) => Some(*v as f32),
			Self::Float(v) => Some(*v as f32),
			Self::Text(s) => {
				let cleaned = s.trim().replace(',', ".");
				cleaned.parse().ok()
			}
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterPages {
	pub pages: Option<PagesField>,
	pub is_paid: Option<bool>,
	pub is_bought: Option<bool>,
	pub server: Option<ServerInfo>,
}

#[derive(Deserialize)]
pub struct ServerInfo {
	pub link: Option<String>,
	pub fallback_link: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum PagesField {
	Flat(Vec<PageItem>),
	Nested(Vec<Vec<PageItem>>),
}

#[derive(Deserialize)]
pub struct PageItem {
	pub link: Option<String>,
}

impl PagesField {
	pub fn flatten(self) -> Vec<PageItem> {
		match self {
			Self::Flat(items) => items,
			Self::Nested(groups) => groups.into_iter().flatten().collect(),
		}
	}
}

pub fn parse_timestamp(value: Option<&str>) -> Option<i64> {
	let raw = value?.trim();
	if raw.is_empty() {
		return None;
	}
	if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
		return Some(dt.timestamp());
	}
	for fmt in [
		"%Y-%m-%dT%H:%M:%S%.f",
		"%Y-%m-%dT%H:%M:%S",
		"%Y-%m-%d %H:%M:%S",
	] {
		if let Ok(naive) = NaiveDateTime::parse_from_str(raw, fmt) {
			return Some(naive.and_utc().timestamp());
		}
	}
	if let Ok(date) =
		NaiveDate::parse_from_str(&raw.chars().take(10).collect::<String>(), "%Y-%m-%d")
	{
		return Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp());
	}
	None
}

fn format_day(iso: &str) -> String {
	if let Some(ts) = parse_timestamp(Some(iso))
		&& let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0)
	{
		return dt.format("%d.%m.%Y").to_string();
	}
	iso.chars().take(10).collect()
}

fn type_name_card(title_type: Option<&TypeField>) -> Option<String> {
	match title_type {
		Some(TypeField::Object(NamedId { name: Some(n), .. })) => Some(n.clone()),
		Some(TypeField::String(s)) => Some(s.clone()),
		_ => None,
	}
}

fn viewer_for(type_name: Option<&str>) -> Viewer {
	match type_name {
		Some(n) if n.contains("анхва") || n.contains("аньхуа") || n.contains("еб") => {
			Viewer::Webtoon
		}
		Some(n) if n.contains("ападн") => Viewer::LeftToRight,
		_ => Viewer::RightToLeft,
	}
}

fn status_from(status: Option<&NamedId>) -> MangaStatus {
	match status.and_then(|s| s.id).or_else(|| {
		status.and_then(|s| match s.name.as_deref() {
			Some("Закончен") => Some(1),
			Some("Продолжается") => Some(2),
			Some("Заморожен") => Some(3),
			Some("Анонс") => Some(5),
			Some("Лицензировано") => Some(6),
			_ => None,
		})
	}) {
		Some(1) | Some(6) => MangaStatus::Completed,
		Some(2) => MangaStatus::Ongoing,
		Some(3) => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

fn rating_from(
	is_erotic: bool,
	is_yaoi: bool,
	age: Option<&AgeLimit>,
	genres: Option<&[NamedId]>,
) -> ContentRating {
	let age_id = age.and_then(|a| a.id);
	let age_name = age.and_then(|a| a.name.as_deref()).unwrap_or("");
	let adult_age = age_id == Some(2)
		|| age_name.contains("18")
		|| age_name.eq_ignore_ascii_case("nsfw");
	let teen_age = age_id == Some(1) || age_name.contains("16") || age_name.contains("17");

	let mut suggestive = teen_age;
	let mut nsfw = is_erotic || is_yaoi || adult_age;

	if !nsfw
		&& let Some(genres) = genres
	{
		for genre in genres {
			let id = genre.id;
			let name = genre.name.as_deref().unwrap_or("");
			let lower = name.to_lowercase();
			// Remanga genre ids: 40 Этти, 41 Юри, 43 Яой
			if id == Some(40) || lower.contains("этти") || lower.contains("ecchi") {
				suggestive = true;
			}
			if id == Some(43)
				|| id == Some(41)
				|| lower.contains("яой")
				|| lower.contains("yaoi")
				|| lower.contains("юри")
				|| lower.contains("yuri")
				|| lower.contains("эро")
				|| lower.contains("hentai")
				|| lower.contains("хент")
			{
				nsfw = true;
				break;
			}
		}
	}

	if nsfw {
		ContentRating::NSFW
	} else if suggestive {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn strip_html(input: &str) -> String {
	let mut out = String::new();
	let mut in_tag = false;
	for ch in input.chars() {
		match ch {
			'<' => in_tag = true,
			'>' => in_tag = false,
			_ if !in_tag => out.push(ch),
			_ => {}
		}
	}
	out.replace("&nbsp;", " ")
		.replace("&amp;", "&")
		.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&quot;", "\"")
		.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
}

fn pick_title(main: Option<String>, secondary: Option<String>, fallback: &str) -> String {
	if english_titles() {
		secondary
			.filter(|s| !s.is_empty())
			.or(main)
			.unwrap_or_else(|| fallback.into())
	} else {
		main.filter(|s| !s.is_empty())
			.or(secondary)
			.unwrap_or_else(|| fallback.into())
	}
}

impl TitleCard {
	pub fn into_manga(self) -> Option<Manga> {
		let key = self.dir.filter(|s| !s.is_empty())?;
		let title = pick_title(self.main_name, self.secondary_name, &key);
		let tname = type_name_card(self.title_type.as_ref());
		let cover = self
			.cover
			.as_ref()
			.and_then(Cover::best)
			.or_else(|| self.img.as_ref().and_then(Cover::best));
		let url = format!("{SITE_URL}/manga/{key}");
		Some(Manga {
			key,
			title,
			cover,
			url: Some(url),
			status: status_from(self.status.as_ref()),
			content_rating: rating_from(
				self.is_erotic.unwrap_or(false),
				self.is_yaoi.unwrap_or(false),
				self.age_limit.as_ref(),
				self.genres.as_deref(),
			),
			viewer: viewer_for(tname.as_deref()),
			..Default::default()
		})
	}
}

impl TitleDetail {
	pub fn into_manga(self, existing: Option<Manga>) -> Manga {
		let key = self
			.dir
			.clone()
			.or_else(|| existing.as_ref().map(|m| m.key.clone()))
			.unwrap_or_default();
		let mut manga = existing.unwrap_or_default();
		manga.key = key.clone();
		manga.title = pick_title(self.main_name, self.secondary_name, &key);
		manga.cover = self.cover.as_ref().and_then(Cover::best).or(manga.cover);
		manga.description = self
			.description
			.map(|d| strip_html(&d))
			.or(manga.description);
		manga.url = Some(format!("{SITE_URL}/manga/{key}"));
		manga.status = status_from(self.status.as_ref());
		manga.content_rating = rating_from(
			self.is_erotic.unwrap_or(false),
			self.is_yaoi.unwrap_or(false),
			self.age_limit.as_ref(),
			self.genres.as_deref(),
		);
		manga.viewer = viewer_for(self.title_type.as_ref().and_then(|t| t.name.as_deref()));

		let mut tags = Vec::new();
		if let Some(genres) = self.genres {
			for g in genres {
				if let Some(name) = g.name.filter(|s| !s.is_empty()) {
					tags.push(name);
				}
			}
		}
		if let Some(categories) = self.categories {
			for c in categories {
				if let Some(name) = c.name.filter(|s| !s.is_empty()) {
					tags.push(name);
				}
			}
		}
		if !tags.is_empty() {
			manga.tags = Some(tags);
		}

		if let Some(creators) = self.creators {
			let authors = creators.into_author_names();
			if !authors.is_empty() {
				manga.authors = Some(authors);
			}
		}

		manga
	}

	pub fn branches(&self) -> &[Branch] {
		self.branches.as_deref().unwrap_or(&[])
	}
}

impl ApiChapter {
	pub fn into_chapter(self, manga_key: &str, branch_label: Option<&str>) -> Chapter {
		let paid = self.is_paid.unwrap_or(false);
		let bought = self.is_bought.unwrap_or(false);
		let locked = paid && !bought;

		let mut title_parts = Vec::new();
		if let Some(name) = self.name.filter(|s| !s.is_empty() && s != "null") {
			title_parts.push(name);
		}
		if show_paid_info() && locked {
			if let Some(price) = self.price.map(|p| p.as_string()).filter(|s| !s.is_empty()) {
				title_parts.push(format!("{price} RM"));
			} else {
				title_parts.push("Платно".into());
			}
			if let Some(free_at) = self.pub_date.as_deref().filter(|s| !s.is_empty()) {
				title_parts.push(format!("бесплатно с {}", format_day(free_at)));
			}
		}

		let scanlators = {
			let mut names = self
				.publishers
				.unwrap_or_default()
				.into_iter()
				.filter_map(|p| p.name)
				.collect::<Vec<_>>();
			if let Some(label) = branch_label.filter(|s| !s.is_empty())
				&& !names.iter().any(|n| n == label)
			{
				names.push(label.into());
			}
			if names.is_empty() { None } else { Some(names) }
		};

		// Stable numeric identity is required for Aidoku history / continue reading.
		let chapter_number = self
			.chapter
			.as_ref()
			.and_then(|c| c.as_f32())
			.or_else(|| self.index.map(|i| i as f32));

		Chapter {
			key: self.id.to_string(),
			title: if title_parts.is_empty() {
				None
			} else {
				Some(title_parts.join(" · "))
			},
			chapter_number,
			volume_number: self.tome.map(|t| t as f32),
			date_uploaded: parse_timestamp(self.upload_date.as_deref())
				.or_else(|| parse_timestamp(self.pub_date.as_deref())),
			scanlators,
			url: Some(format!("{SITE_URL}/manga/{manga_key}/{}", self.id)),
			locked,
			..Default::default()
		}
	}
}
