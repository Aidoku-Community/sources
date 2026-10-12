use crate::{NOVEL_PREFIX, QUERY_LIMIT, WEB_URL};
use aidoku::{
	Chapter, ContentRating, Listing, Manga, MangaPageResult, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::std::parse_date_with_options,
	prelude::format,
};
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

fn flex_optional<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
	Option::<Value>::deserialize(d).map(|opt| match opt {
		Some(Value::String(s)) => Some(s),
		Some(Value::Number(n)) => Some(n.to_string()),
		_ => None,
	})
}

#[derive(Deserialize)]
pub struct ApiOuter {
	#[serde(default, deserialize_with = "flex_optional")]
	pub data: Option<String>,
	#[serde(rename = "errorMsg")]
	pub error_msg: Option<String>,
}

impl ApiOuter {
	// some backends append a php log after the json
	pub fn parse(body: &str) -> serde_json::Result<Self> {
		Self::deserialize(&mut serde_json::Deserializer::from_str(body))
	}
}

#[derive(Deserialize, Serialize)]
pub struct AuthData {
	#[serde(alias = "jwttoken")]
	pub jwt_token: String,
}

impl AuthData {
	pub fn is_valid(&self) -> bool {
		!self.jwt_token.trim().is_empty()
	}
}

fn is_search_term(keyword: &str) -> bool {
	keyword.chars().count() < QUERY_LIMIT
		&& !keyword.contains(|c: char| c.is_whitespace() || "+-<>()~*\"@".contains(c))
}

#[derive(Default)]
pub struct BlockState {
	keywords: Vec<String>,
	ids: Vec<String>,
}

impl BlockState {
	pub fn new(entries: impl Iterator<Item = String>) -> Self {
		let mut state = Self::default();
		for entry in entries {
			// ids as the official app shows them: JM123 (or 123) for albums, NVL123 for novels
			let id = ["nvl", "jm"]
				.into_iter()
				.find_map(|prefix| entry.strip_prefix(prefix))
				.unwrap_or(&entry);
			let list = if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
				&mut state.ids
			} else {
				&mut state.keywords
			};
			if !list.contains(&entry) {
				list.push(entry);
			}
		}
		state
	}

	pub fn blocks(&self, manga: &Manga) -> bool {
		let fields = [manga.title.as_str()]
			.into_iter()
			.chain(manga.description.as_deref())
			.chain(manga.authors.iter().flatten().map(String::as_str))
			.chain(manga.tags.iter().flatten().map(String::as_str));
		self.is_blocked(&manga.key, fields)
	}

	pub fn exclusions(&self, limit: usize) -> Option<String> {
		self.terms("-", limit)
	}

	pub fn lookup(&self) -> Option<String> {
		let query = self.terms("", QUERY_LIMIT)?;
		// a lone one-character term matches nothing unless it's required
		Some(if query.contains(' ') {
			query
		} else {
			format!("+{query}")
		})
	}

	pub fn hide(&mut self, ids: impl Iterator<Item = String>) {
		self.ids.extend(ids);
	}

	fn terms(&self, op: &str, limit: usize) -> Option<String> {
		let mut query = String::new();
		let mut used = 0;
		for keyword in self
			.keywords
			.iter()
			.filter(|keyword| is_search_term(keyword))
		{
			let len = keyword.chars().count() + op.len() + usize::from(!query.is_empty());
			if used + len > limit {
				continue;
			}
			used += len;
			if !query.is_empty() {
				query.push(' ');
			}
			query.push_str(op);
			query.push_str(keyword);
		}
		(!query.is_empty()).then_some(query)
	}

	pub fn is_blocked<'a>(&self, key: &str, fields: impl IntoIterator<Item = &'a str>) -> bool {
		self.blocks_key(key) || self.matches(fields)
	}

	pub fn blocks_key(&self, key: &str) -> bool {
		match key.strip_prefix(NOVEL_PREFIX) {
			Some(id) => self.ids.iter().any(|e| e.strip_prefix("nvl") == Some(id)),
			None => self
				.ids
				.iter()
				.any(|e| e.strip_prefix("jm").unwrap_or(e) == key),
		}
	}

	pub fn matches<'a>(&self, fields: impl IntoIterator<Item = &'a str>) -> bool {
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

#[derive(Deserialize)]
struct CategoryRef {
	title: Option<String>,
}

#[derive(Deserialize)]
pub struct ComicItem {
	#[serde(deserialize_with = "flex_string")]
	pub id: String,
	name: Option<String>,
	author: Option<String>,
	image: Option<String>,
	category: Option<CategoryRef>,
	category_sub: Option<CategoryRef>,
}

#[derive(Deserialize)]
pub struct SearchResp {
	#[serde(deserialize_with = "flex_i32")]
	total: i32,
	#[serde(default, alias = "list")]
	pub content: Vec<ComicItem>,
	#[serde(default, deserialize_with = "flex_optional")]
	pub redirect_aid: Option<String>,
}

fn cover_url(cdn_base: &str, id: &str) -> String {
	format!("{cdn_base}/media/albums/{id}_3x4.jpg")
}

fn image_url(cdn_base: &str, path: String) -> String {
	if path.starts_with("http") {
		path
	} else {
		format!("{cdn_base}{path}")
	}
}

impl SearchResp {
	pub fn into_page_result(
		self,
		page: i32,
		page_size: i32,
		cdn_base: &str,
		ctx: &BlockState,
	) -> MangaPageResult {
		MangaPageResult {
			has_next_page: page * page_size < self.total,
			entries: manga_list(self.content, cdn_base, ctx, ComicItem::into_manga),
		}
	}
}

pub fn manga_list(
	items: Vec<ComicItem>,
	cdn_base: &str,
	ctx: &BlockState,
	convert: fn(ComicItem, &str) -> Manga,
) -> Vec<Manga> {
	// ids are checked on the converted key, which tells albums and novels apart
	items
		.into_iter()
		.filter(|item| !ctx.matches(item.block_fields()))
		.map(|item| convert(item, cdn_base))
		.filter(|manga| !ctx.blocks_key(&manga.key))
		.collect()
}

impl ComicItem {
	pub fn block_fields(&self) -> impl Iterator<Item = &str> {
		[self.name.as_deref(), self.author.as_deref()]
			.into_iter()
			.flatten()
			.chain(self.category.as_ref().and_then(|c| c.title.as_deref()))
			.chain(self.category_sub.as_ref().and_then(|c| c.title.as_deref()))
	}

	pub fn into_manga(self, cdn_base: &str) -> Manga {
		Manga {
			cover: Some(cover_url(cdn_base, &self.id)),
			key: self.id,
			title: self.name.unwrap_or_default(),
			content_rating: ContentRating::NSFW,
			..Default::default()
		}
	}

	pub fn into_novel(self, cdn_base: &str) -> Manga {
		Manga {
			key: format!("{NOVEL_PREFIX}{}", self.id),
			title: self.name.unwrap_or_default(),
			cover: self.image.map(|image| image_url(cdn_base, image)),
			content_rating: ContentRating::NSFW,
			..Default::default()
		}
	}

	pub fn into_home_manga(
		mut self,
		cdn_base: &str,
		convert: fn(ComicItem, &str) -> Manga,
	) -> Manga {
		let authors = self
			.author
			.take()
			.filter(|author| !author.trim().is_empty())
			.map(|author| vec![author]);
		Manga {
			authors,
			..convert(self, cdn_base)
		}
	}
}

#[derive(Deserialize)]
pub struct AlbumResp {
	name: Option<String>,
	author: Option<Vec<String>>,
	addtime: Option<String>,
	description: Option<String>,
	tags: Option<Vec<String>>,
	works: Option<Vec<String>>,
	actors: Option<Vec<String>>,
	series: Option<Vec<SeriesItem>>,
	purchased: Option<Value>,
}

#[derive(Deserialize)]
struct SeriesItem {
	#[serde(deserialize_with = "flex_string")]
	id: String,
	name: Option<String>,
	#[serde(default, deserialize_with = "flex_f32")]
	sort: f32,
}

#[derive(Deserialize)]
pub struct ChapterResp {
	#[serde(deserialize_with = "flex_string")]
	id: String,
	pub images: Vec<String>,
}

impl ChapterResp {
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
			.chain(self.actors.iter().flatten())
			.map(String::as_str)
	}

	fn block_fields(&self) -> impl Iterator<Item = &str> {
		[self.name.as_deref(), self.description.as_deref()]
			.into_iter()
			.flatten()
			.chain(self.author.iter().flatten().map(String::as_str))
			.chain(self.labels())
	}

	pub fn is_blocked(&self, key: &str, ctx: &BlockState) -> bool {
		ctx.is_blocked(key, self.block_fields())
	}

	fn content_rating(&self) -> ContentRating {
		rating(self.labels())
	}

	pub fn is_unpaid(&self) -> bool {
		self.purchased == Some(Value::Bool(false))
	}

	pub fn into_entry(self, key: &str, cdn_base: &str) -> Manga {
		let content_rating = self.content_rating();
		Manga {
			key: key.into(),
			title: self.name.unwrap_or_default(),
			cover: Some(cover_url(cdn_base, key)),
			content_rating,
			..Default::default()
		}
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
			tags: unique(
				self.tags
					.into_iter()
					.flatten()
					.chain(self.works.into_iter().flatten())
					.chain(self.actors.into_iter().flatten())
					.filter_map(|tag| {
						let tag = tag.trim();
						(!tag.is_empty()).then(|| tag.into())
					}),
			),
			url: Some(format!("{WEB_URL}/album/{key}")),
			status,
			content_rating,
			viewer,
			..Default::default()
		}
	}

	pub fn take_chapters(&mut self, manga_key: &str) -> Vec<Chapter> {
		let date_uploaded = self.addtime.as_deref().and_then(|time| time.parse().ok());
		let series = self.series.take().unwrap_or_default();
		if series.is_empty() {
			return vec![Chapter {
				key: manga_key.into(),
				chapter_number: Some(1.0),
				date_uploaded,
				url: Some(format!("{WEB_URL}/photo/{manga_key}")),
				..Default::default()
			}];
		}

		let names: Vec<_> = series
			.iter()
			.map(|item| parse_name(item.name.as_deref().unwrap_or_default()))
			.collect();
		let labelled = names.iter().any(|(number, _)| number.is_some());
		let mut prev: Option<(f32, f32)> = None;
		let forward: Vec<_> = names
			.iter()
			.map(|name| match name {
				(Some(n), _) => {
					prev = Some((*n, 0.0));
					None
				}
				(None, None) => prev.as_mut().map(|(n, gap)| {
					*gap += 1.0;
					*n + *gap
				}),
				_ => None,
			})
			.collect();
		let newest = series.len() - 1;
		let mut next: Option<(f32, f32)> = None;
		series
			.into_iter()
			.zip(names)
			.zip(forward)
			.enumerate()
			.rev()
			.map(|(i, ((item, (label, title)), forward))| {
				let chapter_number = match label {
					_ if !labelled => Some(item.sort),
					Some(n) => {
						next = Some((n, 0.0));
						Some(n)
					}
					None if title.is_none() => {
						let back = next.as_mut().map(|(n, gap)| {
							*gap += 1.0;
							*n - *gap
						});
						forward
							.into_iter()
							.chain(back)
							.reduce(f32::min)
							.filter(|n| *n >= 0.0)
					}
					None => None,
				};
				Chapter {
					url: Some(format!("{WEB_URL}/photo/{}", item.id)),
					key: item.id,
					title,
					chapter_number,
					date_uploaded: date_uploaded.filter(|_| i == newest),
					..Default::default()
				}
			})
			.collect()
	}

	fn viewer(&self) -> Option<Viewer> {
		self.labels().find_map(|label| match label {
			"條漫" | "条漫" | "韓漫" | "韩漫" | "韓國" | "韩国" => {
				Some(Viewer::Webtoon)
			}
			"美漫" => Some(Viewer::LeftToRight),
			_ if label.eq_ignore_ascii_case("webtoon") => Some(Viewer::Webtoon),
			_ => None,
		})
	}

	fn status(&self) -> MangaStatus {
		let mut ongoing = false;
		for tag in self.tags.iter().flatten().map(String::as_str) {
			if matches!(tag, "完結" | "完结" | "已完結" | "已完结") {
				return MangaStatus::Completed;
			}
			ongoing |= matches!(tag, "連載中" | "连载中" | "連載" | "连载");
		}
		if ongoing {
			MangaStatus::Ongoing
		} else {
			MangaStatus::Unknown
		}
	}
}

fn unique(labels: impl Iterator<Item = String>) -> Option<Vec<String>> {
	let mut labels: Vec<_> = labels.enumerate().collect();
	labels.sort_by(|(_, a), (_, b)| a.cmp(b));
	labels.dedup_by(|(_, a), (_, b)| a == b);
	labels.sort_unstable_by_key(|(i, _)| *i);
	(!labels.is_empty()).then(|| labels.into_iter().map(|(_, label)| label).collect())
}

fn rating<'a>(labels: impl IntoIterator<Item = &'a str>) -> ContentRating {
	for label in labels {
		if label == "青年漫" {
			return ContentRating::Suggestive;
		}
		if label == "非H" {
			return ContentRating::Safe;
		}
	}
	ContentRating::NSFW
}

fn leading_number(text: &str) -> Option<(f32, &str)> {
	let rest = text.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
	let number = text
		.strip_suffix(rest)?
		.trim_end_matches('.')
		.parse()
		.ok()?;
	Some((number, rest))
}

fn episode_label(text: &str) -> Option<(f32, &str)> {
	let (number, rest) = leading_number(text.trim_start())?;
	Some((number, rest.trim_start().strip_prefix(['話', '话'])?))
}

fn parse_name(name: &str) -> (Option<f32>, Option<String>) {
	let name = name.trim();
	let labelled = match name.strip_prefix('第') {
		Some(body) => episode_label(body),
		None => episode_label(name).or_else(|| {
			leading_number(name)
				.filter(|(_, rest)| !rest.starts_with(char::is_alphanumeric))
				.map(|(number, rest)| {
					let is_range = rest.starts_with(['-', '~', '～']);
					(number, if is_range { name } else { rest })
				})
		}),
	};
	let (number, rest) = match labelled {
		Some((number, rest)) => (Some(number), rest),
		None => (
			name.split('第')
				.skip(1)
				.find_map(|part| episode_label(part).map(|(number, _)| number)),
			name,
		),
	};
	let title = rest
		.trim_start_matches(|c: char| {
			c.is_whitespace() || matches!(c, '-' | ':' | '：' | '.' | '、')
		})
		.trim_end();
	(number, (!title.is_empty()).then(|| title.into()))
}

#[derive(Deserialize)]
pub struct NovelResp {
	name: Option<String>,
	author: Option<String>,
	images: Option<String>,
	description: Option<String>,
	tags: Option<Vec<String>>,
	serial_status: Option<String>,
	#[serde(default)]
	series: Vec<NovelChapterItem>,
}

#[derive(Deserialize)]
struct NovelChapterItem {
	#[serde(rename = "NCID", deserialize_with = "flex_string")]
	id: String,
	title: Option<String>,
	#[serde(default, deserialize_with = "flex_f32")]
	sort: f32,
	on_at: Option<String>,
	is_need_buy_nc: Option<String>,
	#[serde(default)]
	purchased: bool,
}

#[derive(Deserialize)]
pub struct NovelChapterResp {
	#[serde(deserialize_with = "flex_string")]
	nid: String,
	pub content: Option<String>,
}

impl NovelChapterResp {
	pub fn novel_key(&self) -> Option<String> {
		(!self.nid.is_empty() && self.nid != "0").then(|| format!("{NOVEL_PREFIX}{}", self.nid))
	}
}

impl NovelResp {
	pub fn is_blocked(&self, key: &str, ctx: &BlockState) -> bool {
		let fields = [
			self.name.as_deref(),
			self.author.as_deref(),
			self.description.as_deref(),
		];
		ctx.is_blocked(
			key,
			fields
				.into_iter()
				.flatten()
				.chain(self.tags.iter().flatten().map(String::as_str)),
		)
	}

	pub fn into_entry(self, key: &str, cdn_base: &str) -> Manga {
		let content_rating = rating(self.tags.iter().flatten().map(String::as_str));
		Manga {
			key: key.into(),
			title: self.name.unwrap_or_default(),
			cover: self.images.map(|image| image_url(cdn_base, image)),
			content_rating,
			..Default::default()
		}
	}

	pub fn into_manga(self, key: &str, cdn_base: &str) -> Manga {
		let content_rating = rating(self.tags.iter().flatten().map(String::as_str));
		let status = match self.serial_status.as_deref() {
			Some("Ongoing") => MangaStatus::Ongoing,
			Some("Completed") => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};
		Manga {
			key: key.into(),
			title: self.name.unwrap_or_default(),
			cover: self.images.map(|image| image_url(cdn_base, image)),
			authors: self
				.author
				.filter(|author| !author.trim().is_empty())
				.map(|author| vec![author]),
			description: self.description.filter(|s| !s.trim().is_empty()),
			tags: self.tags.filter(|tags| !tags.is_empty()),
			url: Some(format!("{WEB_URL}/{key}")),
			status,
			content_rating,
			..Default::default()
		}
	}

	pub fn is_unpaid(&self, chapter_key: &str) -> bool {
		self.series.iter().any(|item| {
			item.id == chapter_key && item.is_need_buy_nc.as_deref() == Some("1") && !item.purchased
		})
	}

	pub fn take_chapters(&mut self) -> Vec<Chapter> {
		let series = core::mem::take(&mut self.series);
		let names: Vec<_> = series
			.iter()
			.map(|item| parse_name(item.title.as_deref().unwrap_or_default()))
			.collect();
		let labelled = names.iter().any(|(number, _)| number.is_some());
		series
			.into_iter()
			.zip(names)
			.rev()
			.map(|(item, (number, title))| Chapter {
				chapter_number: if labelled { number } else { Some(item.sort) },
				title,
				date_uploaded: item.on_at.as_deref().and_then(|date| {
					parse_date_with_options(date, "yyyy-MM-dd HH:mm:ss", "en_US", "Asia/Taipei")
				}),
				url: Some(format!("{WEB_URL}/novelchapters/{}", item.id)),
				key: item.id,
				..Default::default()
			})
			.collect()
	}
}

#[derive(Deserialize)]
pub struct DomainRefreshResp {
	#[serde(rename = "Server", default)]
	pub server: Vec<String>,
}

#[derive(Deserialize)]
pub struct SettingData {
	pub img_host: Option<String>,
}

#[derive(Deserialize)]
pub struct PromoteGroup {
	#[serde(deserialize_with = "flex_string")]
	id: String,
	pub title: Option<String>,
	#[serde(rename = "type")]
	group_type: Option<String>,
	slug: Option<String>,
	pub content: Vec<ComicItem>,
}

impl PromoteGroup {
	pub fn is_visible(&self) -> bool {
		self.group_type.as_deref() != Some("library") && self.slug.as_deref() != Some("another")
	}

	pub fn is_novels(&self) -> bool {
		self.group_type.as_deref() == Some("novels")
	}

	pub fn listing(&self) -> Option<Listing> {
		let slug = self.slug.as_deref().unwrap_or_default();
		let id = match self.group_type.as_deref()? {
			"promote" => format!("promo:{}", self.id),
			"novels" => "novels".into(),
			kind if kind.contains("category_id") && !slug.is_empty() => format!("cat:{slug}"),
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
}
