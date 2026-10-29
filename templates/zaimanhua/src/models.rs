use crate::PAGE_SIZE;
use aidoku::{
	Filter, Link, Manga, MangaPageResult, MangaStatus, Result, SelectFilter,
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
	serde::Deserialize,
};
use core::fmt::Display;

// errors keep http 200 and send `data` as `{}` or `[]`
#[derive(Deserialize)]
pub struct ApiResponse<T> {
	#[serde(default)]
	pub errno: i64,
	pub errmsg: Option<String>,
	data: Option<T>,
}

impl<T: Default> ApiResponse<T> {
	pub fn into_result(self) -> Result<T> {
		if self.errno != 0 {
			bail!("{}", self.errmsg.as_deref().unwrap_or("未知错误"));
		}
		Ok(self.data.unwrap_or_default())
	}
}

pub fn manga(key: String, title: String, cover: Option<String>, status: Option<&str>) -> Manga {
	Manga {
		key,
		title,
		cover,
		status: status.map(parse_status).unwrap_or_default(),
		..Default::default()
	}
}

pub fn link(manga: Manga, authors: Option<String>) -> Link {
	Manga {
		authors: authors.map(|authors| vec![authors]),
		..manga
	}
	.into()
}

pub fn parse_status(status: &str) -> MangaStatus {
	match status {
		"连载中" => MangaStatus::Ongoing,
		"已完结" | "短篇" => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	}
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ListData {
	#[serde(alias = "comicList", alias = "novelList", alias = "subList")]
	pub list: Vec<ListItem>,
	#[serde(alias = "totalNum")]
	total: i32,
}

impl ListData {
	pub fn into_page_result(self, page: i32) -> MangaPageResult {
		MangaPageResult {
			// some accounts get comic totals larger than the list, so an empty page also ends it
			has_next_page: !self.list.is_empty() && self.total > page * PAGE_SIZE,
			entries: self.list.into_iter().map(ListItem::into_manga).collect(),
		}
	}
}

#[derive(Deserialize)]
pub struct ListItem {
	pub id: i64,
	#[serde(alias = "name")]
	title: String,
	cover: Option<String>,
	pub authors: Option<String>,
	pub comic_py: Option<String>,
	status: Option<String>,
	pub last_update_volume_id: Option<i64>,
	pub last_update_volume_name: Option<String>,
	pub last_update_chapter_id: Option<i64>,
	pub last_update_chapter_name: Option<String>,
	pub last_updatetime: Option<i64>,
}

impl ListItem {
	pub fn into_manga(self) -> Manga {
		manga(
			self.id.to_string(),
			self.title,
			self.cover,
			self.status.as_deref(),
		)
	}

	pub fn into_link(mut self) -> Link {
		let authors = self.authors.take();
		link(self.into_manga(), authors)
	}
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct UnifiedData {
	list: Vec<UnifiedItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnifiedItem {
	title: String,
	cover_url: Option<String>,
	status: Option<String>,
	authors: Option<Vec<String>>,
	types: Option<Vec<String>>,
	description: Option<String>,
}

impl UnifiedData {
	pub fn into_manga(self, id: impl Display) -> Option<Manga> {
		let item = self.list.into_iter().next()?;
		Some(manga(
			id.to_string(),
			item.title,
			item.cover_url,
			item.status.as_deref(),
		))
	}

	pub fn into_featured(self, id: impl Display) -> Option<Manga> {
		let item = self.list.into_iter().next()?;
		Some(Manga {
			authors: item.authors,
			description: item.description,
			tags: item
				.types
				.map(|types| types.into_iter().map(tag_name).collect()),
			..manga(
				id.to_string(),
				item.title,
				item.cover_url,
				item.status.as_deref(),
			)
		})
	}
}

pub fn tag_name(name: String) -> String {
	match name.as_str() {
		"青年漫画" => "男青漫画".into(),
		"ゆり" => "百合".into(),
		_ => name,
	}
}

pub fn tag_filter(
	id: &'static str,
	title: &'static str,
	is_genre: bool,
	tags: Vec<ClassifyTag>,
) -> Filter {
	let (options, ids) = tags
		.into_iter()
		.map(|tag| (tag_name(tag.tag_name).into(), tag.tag_id.to_string().into()))
		.unzip();
	SelectFilter {
		id: id.into(),
		title: Some(title.into()),
		is_genre,
		uses_tag_style: true,
		options,
		ids: Some(ids),
		..Default::default()
	}
	.into()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyTag {
	pub tag_id: i64,
	pub tag_name: String,
}

#[derive(Deserialize)]
pub struct RecommendCategory {
	pub category_id: i64,
	pub data: Vec<RecommendItem>,
}

#[derive(Deserialize)]
pub struct RecommendItem {
	pub obj_id: i64,
	pub title: String,
	#[serde(rename = "type")]
	pub kind: i32,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct NewsData {
	pub news_list: Vec<NewsItem>,
}

#[derive(Deserialize)]
pub struct NewsItem {
	pub article_id: i64,
	pub title: String,
	pub row_pic_url: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct LoginData {
	pub user: Option<LoginUser>,
}

#[derive(Deserialize)]
pub struct LoginUser {
	pub token: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct UserInfoData {
	pub user_info: Option<UserInfo>,
}

#[derive(Deserialize)]
pub struct UserInfo {
	pub nickname: String,
	pub user_level: i64,
	pub is_sign: bool,
	#[serde(rename = "isMember")]
	pub is_member: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct TaskListData {
	pub task: Option<TaskGroup>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroup {
	#[serde(default)]
	pub day_task: Vec<TaskItem>,
	#[serde(default)]
	pub new_user_task: Vec<TaskItem>,
	pub sum_sign_task: Option<SumSignTask>,
}

#[derive(Deserialize)]
pub struct SumSignTask {
	#[serde(default)]
	pub list: Vec<TaskItem>,
}

#[derive(Deserialize)]
pub struct TaskItem {
	pub id: i64,
	pub status: i64,
}
