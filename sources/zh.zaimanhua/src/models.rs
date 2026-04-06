use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	serde::Deserialize,
};

#[derive(Deserialize)]
pub struct ApiResponse<T> {
	pub errno: Option<i64>,
	pub errmsg: Option<String>,
	pub data: Option<T>,
}

#[derive(Deserialize)]
pub struct SearchData {
	pub list: Vec<SearchItem>,
}

#[derive(Deserialize, Clone)]
pub struct SearchItem {
	pub id: i64, // Primary ID (comic_id is always 0 in search)
	pub title: String,
	pub cover: Option<String>,
	pub authors: Option<String>,
	pub status: Option<String>,
}

impl SearchItem {
	pub fn matches_author(&self, target_author: &str) -> bool {
		matches_author_str(self.authors.as_deref().unwrap_or(""), target_author)
	}
}

impl From<SearchItem> for Manga {
	fn from(item: SearchItem) -> Self {
		let key = item.id.to_string();
		let status = item.status.as_deref().map(parse_status).unwrap_or_default();

		Self {
			key,
			title: item.title,
			cover: item.cover,
			status,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct FilterData {
	#[serde(rename = "comicList")]
	pub comic_list: Vec<FilterItem>,
}

#[derive(Deserialize)]
pub struct FilterItem {
	pub id: i64,
	pub name: String,
	pub cover: Option<String>,
	pub authors: Option<String>,
	pub status: Option<String>,
	pub last_update_chapter_name: Option<String>,
	pub last_update_chapter_id: Option<i64>,
	pub last_updatetime: Option<i64>,
}

impl FilterItem {
	pub fn into_manga_with_chapter(self) -> MangaWithChapter {
		let key = self.id.to_string();
		let last_ch_id = self.last_update_chapter_id.unwrap_or(0);
		let chapter_key = format!("{key}/{last_ch_id}");
		let status = self.status.as_deref().map(parse_status).unwrap_or_default();

		MangaWithChapter {
			manga: Manga {
				key,
				title: self.name,
				cover: self.cover,
				status,
				..Default::default()
			},
			chapter: Chapter {
				key: chapter_key,
				title: self.last_update_chapter_name,
				date_uploaded: self.last_updatetime,
				..Default::default()
			},
		}
	}
}

impl From<FilterItem> for Manga {
	fn from(item: FilterItem) -> Self {
		let key = item.id.to_string();
		let authors = item.authors.map(|a| vec![a]);
		let status = item.status.as_deref().map(parse_status).unwrap_or_default();

		Self {
			key,
			title: item.name,
			cover: item.cover,
			authors,
			status,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct RankItem {
	pub comic_id: i64,
	pub title: String,
	pub cover: Option<String>,
	pub authors: Option<String>,
	pub status: Option<String>,
}

impl From<RankItem> for Manga {
	fn from(item: RankItem) -> Self {
		let key = item.comic_id.to_string();
		let authors = item.authors.map(|a| vec![a]);
		let status = item.status.as_deref().map(parse_status).unwrap_or_default();

		Self {
			key,
			title: item.title,
			cover: item.cover,
			authors,
			status,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct DetailData {
	pub data: Option<MangaDetail>,
}

#[derive(Deserialize)]
pub struct MangaDetail {
	pub title: Option<String>,
	pub cover: Option<String>,
	pub description: Option<String>,
	pub authors: Option<Vec<AuthorTag>>,
	#[serde(alias = "types")]
	pub theme: Option<Vec<ThemeTag>>,
	pub status: Option<Vec<StatusTag>>,
	pub chapters: Option<Vec<ChapterGroup>>,
	pub direction: Option<i32>,
	pub islong: Option<i32>,
	pub comic_py: Option<String>,
	pub hidden: Option<i32>,
	pub is_need_login: Option<i32>,
}

#[derive(Deserialize)]
pub struct AuthorTag {
	pub tag_id: Option<i64>,
	pub tag_name: Option<String>,
}

#[derive(Deserialize)]
pub struct ThemeTag {
	pub tag_name: Option<String>,
}

pub fn normalize_tag_name(name: String) -> String {
	match name.as_str() {
		"青年漫画" => String::from("男青漫画"),
		"ゆり" => String::from("百合"),
		_ => name,
	}
}

#[derive(Deserialize)]
pub struct StatusTag {
	pub tag_name: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterGroup {
	pub data: Vec<ChapterItem>,
	pub title: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub chapter_id: i64,
	pub chapter_title: Option<String>,
	pub updatetime: Option<i64>,
}

impl MangaDetail {
	fn status(&self) -> MangaStatus {
		self.status
			.as_deref()
			.unwrap_or_default()
			.iter()
			.find_map(|s| {
				let st = parse_status(s.tag_name.as_deref()?);
				(st != MangaStatus::Unknown).then_some(st)
			})
			.unwrap_or_default()
	}

	fn content_rating(&self) -> ContentRating {
		if self.hidden.unwrap_or(0) == 1 {
			ContentRating::NSFW
		} else if self.is_need_login.unwrap_or(0) == 1 {
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		}
	}

	fn viewer(&self) -> Viewer {
		match self.islong {
			Some(1) => Viewer::Webtoon,
			_ => match self.direction {
				Some(2) => Viewer::LeftToRight,
				_ => Viewer::RightToLeft,
			},
		}
	}

	pub fn to_chapters(&self, manga_key: &str) -> Vec<Chapter> {
		let comic_py = self.comic_py.as_deref().unwrap_or_default();
		let web_url = crate::WEB_URL;
		let mut chapters = Vec::new();

		if let Some(groups) = &self.chapters {
			for group in groups {
				let group_title = group.title.as_deref().unwrap_or_default();
				for item in &group.data {
					let chapter_id = item.chapter_id.to_string();
					let ch_url = Some(format!(
						"{web_url}/view/{comic_py}/{manga_key}/{chapter_id}"
					));

					chapters.push(Chapter {
						key: format!("{manga_key}/{chapter_id}"),
						title: item.chapter_title.clone(),
						scanlators: Some(vec![group_title.to_string()]),
						date_uploaded: item.updatetime,
						url: ch_url,
						..Default::default()
					});
				}
			}
		}

		let total = chapters.len() as f32;
		for (idx, chapter) in chapters.iter_mut().enumerate() {
			chapter.chapter_number = Some(total - idx as f32);
		}

		chapters
	}

	pub fn into_manga(self, key: String) -> Manga {
		let status = self.status();
		let content_rating = self.content_rating();
		let viewer = self.viewer();

		let authors: Option<Vec<String>> = self
			.authors
			.map(|list| list.into_iter().filter_map(|a| a.tag_name).collect());

		let tags: Option<Vec<String>> = self.theme.map(|list| {
			list.into_iter()
				.filter_map(|t| t.tag_name.map(normalize_tag_name))
				.collect()
		});

		let web_url = crate::WEB_URL;
		let url = Some(format!("{web_url}/details/{key}"));

		Manga {
			key,
			title: self.title.unwrap_or_default(),
			cover: self.cover,
			description: self.description,
			authors,
			tags,
			url,
			status,
			content_rating,
			viewer,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct SubscribeData {
	#[serde(rename = "subList")]
	pub sub_list: Vec<SubscribeItem>,
}

#[derive(Deserialize)]
pub struct SubscribeItem {
	pub id: i64,
	pub name: Option<String>,
	pub cover: Option<String>,
	pub authors: Option<String>,
}

impl From<SubscribeItem> for Manga {
	fn from(item: SubscribeItem) -> Self {
		let key = item.id.to_string();

		Self {
			key,
			title: item.name.unwrap_or_default(),
			cover: item.cover,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterData {
	pub data: ChapterPageData,
}

#[derive(Deserialize)]
pub struct ChapterPageData {
	pub page_url: Option<Vec<String>>,
	pub page_url_hd: Option<Vec<String>>,
}

fn parse_status(status_str: &str) -> MangaStatus {
	match status_str {
		s if s.contains("连载") => MangaStatus::Ongoing,
		s if s.contains("完结") => MangaStatus::Completed,
		s if s.contains("停更") || s.contains("暂停") => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

fn matches_author_str(authors_str: &str, target_author: &str) -> bool {
	if authors_str.is_empty() {
		return false;
	}

	let separators = ['/', ',', '，', '、', '&', ';'];
	let parts = authors_str.split(|c| separators.contains(&c));

	for part in parts {
		let trimmed = part.trim();
		if trimmed.eq_ignore_ascii_case(target_author) {
			return true;
		}
	}

	// Allow loose matching only for specific queries (Multibyte or >3 chars)
	if target_author.len() > 3 || !target_author.is_ascii() {
		return authors_str
			.to_lowercase()
			.contains(&target_author.to_lowercase());
	}

	false
}

pub fn manga_list_from_filter(items: Vec<FilterItem>) -> MangaPageResult {
	let entries: Vec<Manga> = items
		.into_iter()
		.filter(|item| item.id > 0)
		.map(Into::into)
		.collect();
	let has_next_page = !entries.is_empty();
	MangaPageResult {
		entries,
		has_next_page,
	}
}

pub fn manga_list_from_ranks(items: Vec<RankItem>) -> MangaPageResult {
	let entries: Vec<Manga> = items
		.into_iter()
		.filter(|item| item.comic_id > 0)
		.map(Into::into)
		.collect();
	let has_next_page = !entries.is_empty();
	MangaPageResult {
		entries,
		has_next_page,
	}
}

pub fn manga_list_from_subscribes(items: Vec<SubscribeItem>) -> MangaPageResult {
	let entries: Vec<Manga> = items
		.into_iter()
		.filter(|item| item.id > 0)
		.map(Into::into)
		.collect();
	let has_next_page = !entries.is_empty();
	MangaPageResult {
		entries,
		has_next_page,
	}
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
	pub sub_title: Option<String>,
	#[serde(rename = "type")]
	pub item_type: i64,
	pub cover: Option<String>,
}

#[derive(Deserialize)]
pub struct ClassifyData {
	#[serde(rename = "classifyList")]
	pub classify_list: Vec<ClassifyGroup>,
}

#[derive(Deserialize)]
pub struct ClassifyGroup {
	pub id: i64,
	pub list: Vec<ClassifyTag>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyTag {
	pub tag_id: i64,
	pub tag_name: String,
}

#[derive(Deserialize)]
pub struct LoginData {
	pub user: Option<UserToken>,
}

#[derive(Deserialize)]
pub struct UserToken {
	pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct UserInfoData {
	#[serde(rename = "userInfo")]
	pub user_info: Option<UserInfo>,
}

#[derive(Deserialize)]
pub struct TaskListData {
	pub task: Option<TaskGroup>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroup {
	pub day_task: Option<Vec<TaskItem>>,
	pub sum_sign_task: Option<SumSignTask>,
}

#[derive(Deserialize)]
pub struct TaskItem {
	pub id: i64,
	pub status: Option<i64>,
}

#[derive(Deserialize)]
pub struct SumSignTask {
	pub list: Option<Vec<TaskItem>>,
}

#[derive(Deserialize)]
pub struct UserInfo {
	#[serde(rename = "user_level")]
	pub level: Option<i64>,
	pub is_sign: Option<bool>,
}
