use crate::{WEB_URL, chapter};
use aidoku::{
	Chapter, ContentRating, Link, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	serde::{Deserialize, de::IgnoredAny},
};
use zaimanhua_template::models::{ClassifyTag, link, manga, parse_status, tag_name};

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct DetailData {
	pub data: Option<ComicDetail>,
}

#[derive(Deserialize)]
pub struct ComicDetail {
	pub id: i64,
	title: String,
	cover: Option<String>,
	description: Option<String>,
	authors: Option<Vec<Tag>>,
	types: Option<Vec<Tag>>,
	status: Option<Vec<Tag>>,
	chapters: Option<Vec<ChapterGroup>>,
	comic_py: Option<String>,
	direction: Option<i32>,
	islong: Option<i32>,
	hidden: Option<i32>,
	is_need_login: Option<i32>,
	pub is_fee: Option<i32>,
}

#[derive(Deserialize)]
struct Tag {
	tag_id: Option<i64>,
	tag_name: String,
}

#[derive(Deserialize)]
struct ChapterGroup {
	title: Option<String>,
	data: Vec<ChapterItem>,
}

#[derive(Deserialize)]
struct ChapterItem {
	chapter_id: i64,
	chapter_title: String,
	chapter_name: Option<String>,
	updatetime: Option<i64>,
	#[serde(default)]
	is_fee: bool,
}

impl ComicDetail {
	pub fn author_id(&self, name: &str) -> Option<i64> {
		self.authors
			.iter()
			.flatten()
			.find(|tag| tag.tag_name.eq_ignore_ascii_case(name))?
			.tag_id
	}

	pub fn first_author(&self) -> Option<(String, Option<i64>)> {
		let tag = self.authors.as_ref()?.first()?;
		Some((tag.tag_name.clone(), tag.tag_id))
	}

	pub fn take_chapters(&mut self, manga_key: &str, is_member: bool) -> Vec<Chapter> {
		let comic_py = self.comic_py.as_deref().filter(|py| !py.is_empty());
		self.chapters
			.take()
			.unwrap_or_default()
			.into_iter()
			.flat_map(|group| {
				let scanlators = group.title.map(|title| vec![title]);
				let numbers = chapter::comic(group.data.iter().map(|item| {
					let name = item
						.chapter_name
						.as_deref()
						.map(str::trim)
						.filter(|name| !name.is_empty())
						.unwrap_or(&item.chapter_title);
					(item.chapter_title.as_str(), name)
				}));
				group
					.data
					.into_iter()
					.zip(numbers)
					.map(move |(item, chapter)| {
						let chapter_id = item.chapter_id;
						Chapter {
							key: format!("{manga_key}/{chapter_id}"),
							date_uploaded: item.updatetime,
							scanlators: scanlators.clone(),
							url: comic_py
								.map(|py| format!("{WEB_URL}/view/{py}/{manga_key}/{chapter_id}")),
							locked: item.is_fee && !is_member,
							..chapter
						}
					})
			})
			.collect()
	}

	pub fn into_manga(self, key: &str) -> Manga {
		let content_rating = match (self.hidden, self.is_need_login) {
			(Some(1), Some(1)) => ContentRating::NSFW,
			(Some(1), _) | (_, Some(1)) => ContentRating::Suggestive,
			_ => ContentRating::Safe,
		};
		let viewer = match (self.islong, self.direction) {
			(Some(1), _) => Viewer::Webtoon,
			(_, Some(2)) => Viewer::LeftToRight,
			_ => Viewer::RightToLeft,
		};
		Manga {
			key: key.into(),
			title: self.title,
			cover: self.cover,
			authors: self
				.authors
				.map(|tags| tags.into_iter().map(|tag| tag.tag_name).collect()),
			description: self.description,
			url: Some(format!("{WEB_URL}/details/{key}")),
			tags: self
				.types
				.map(|tags| tags.into_iter().map(|tag| tag_name(tag.tag_name)).collect()),
			status: self
				.status
				.into_iter()
				.flatten()
				.map(|tag| parse_status(&tag.tag_name))
				.find(|status| *status != MangaStatus::Unknown)
				.unwrap_or_default(),
			content_rating,
			viewer,
			..Default::default()
		}
	}
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ChapterData {
	pub data: Option<ChapterPages>,
}

#[derive(Deserialize)]
pub struct ChapterPages {
	pub page_url: Option<Vec<String>>,
	pub page_url_hd: Option<Vec<String>>,
	#[serde(default)]
	pub comment_count: i64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ViewpointData {
	list: Option<Vec<Vec<ViewpointField>>>,
}

// a viewpoint is an array whose last field is its text
#[derive(Deserialize)]
#[serde(untagged)]
enum ViewpointField {
	Text(String),
	Other(IgnoredAny),
}

impl ViewpointData {
	pub fn into_texts(self) -> impl Iterator<Item = String> {
		self.list
			.into_iter()
			.flatten()
			.filter_map(|mut fields| match fields.pop()? {
				ViewpointField::Text(text) => Some(text),
				ViewpointField::Other(_) => None,
			})
	}
}

#[derive(Deserialize)]
pub struct RankItem {
	comic_id: i64,
	title: String,
	cover: Option<String>,
	authors: Option<String>,
	status: Option<String>,
}

impl From<RankItem> for Manga {
	fn from(item: RankItem) -> Self {
		manga(
			item.comic_id.to_string(),
			item.title,
			item.cover,
			item.status.as_deref(),
		)
	}
}

impl From<RankItem> for Link {
	fn from(mut item: RankItem) -> Self {
		let authors = item.authors.take();
		link(item.into(), authors)
	}
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ClassifyData {
	pub classify_list: Vec<ClassifyGroup>,
}

#[derive(Deserialize)]
pub struct ClassifyGroup {
	pub id: i64,
	pub list: Vec<ClassifyTag>,
}
