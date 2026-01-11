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
	pub total: Option<i64>,
}

#[derive(Deserialize)]
pub struct SearchItem {
	pub id: i64, // Primary ID (comic_id is always 0 in search)
	pub title: String,
	pub cover: Option<String>,
	pub authors: Option<String>,
	pub status: Option<String>,
}

impl From<SearchItem> for Manga {
	fn from(item: SearchItem) -> Self {
		let key = item.id.to_string();
		let authors = item.authors.map(|a| vec![a]);
		let status = item.status.as_deref().map(parse_status).unwrap_or_default();

		Self {
			key,
			title: item.title,
			cover: item.cover,
			authors,
			status,
			content_rating: ContentRating::Safe,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct FilterData {
	#[serde(rename = "comicList")]
	pub comic_list: Vec<FilterItem>,
}

#[derive(Deserialize, Clone)]
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
		// Extract chapter-specific data before consuming self
		let chapter_id = self.last_update_chapter_id.unwrap_or(0).to_string();
		let chapter_key = format!("{}/{}", self.id, chapter_id);
		let chapter_title = self.last_update_chapter_name.clone();
		let chapter_date = self.last_updatetime;

		// Now consume self for Manga
		let manga: Manga = self.into();

		MangaWithChapter {
			manga,
			chapter: Chapter {
				key: chapter_key,
				title: chapter_title,
				date_uploaded: chapter_date,
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
			content_rating: ContentRating::Safe,
			..Default::default()
		}
	}
}

// === Rank API ===
// Response: data[] with comic_id, title, authors, cover, status

#[derive(Deserialize, Clone)]
pub struct RankItem {
	pub comic_id: i64,
	pub title: String,
	pub cover: Option<String>,
	pub authors: Option<String>,
	pub status: Option<String>,
	pub num: Option<i64>,
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
			content_rating: ContentRating::Safe,
			..Default::default()
		}
	}
}

// === Manga Details ===
// Response: data.data with id, title, cover, description, authors[], types[], status[], chapters[]

#[derive(Deserialize)]
pub struct DetailData {
	pub data: Option<MangaDetail>,
}

#[derive(Deserialize, Clone)]
pub struct MangaDetail {
	#[allow(dead_code)]
	pub id: i64,
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
}

#[derive(Deserialize, Clone)]
pub struct AuthorTag {
	pub tag_id: Option<i64>,
	pub tag_name: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct ThemeTag {
	pub tag_name: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct StatusTag {
	pub tag_name: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct ChapterGroup {
	pub data: Vec<ChapterItem>,
	pub title: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct ChapterItem {
	pub chapter_id: i64,
	pub chapter_title: Option<String>,
	pub updatetime: Option<i64>,
}

impl MangaDetail {
	pub fn into_manga(self, key: String) -> Manga {
		let authors: Option<Vec<String>> = self
			.authors
			.map(|list| list.into_iter().filter_map(|a| a.tag_name).collect());

		let tags: Option<Vec<String>> = self
			.theme
			.map(|list| list.into_iter().filter_map(|t| t.tag_name).collect());

		let mut status = MangaStatus::Unknown;
		let mut content_rating = ContentRating::Safe;

		if let Some(status_list) = self.status {
			for s in status_list {
				if let Some(name) = s.tag_name {
					let name_str = name.as_str();
					let parsed_status = parse_status(name_str);
					if parsed_status != MangaStatus::Unknown {
						status = parsed_status;
					}
					if name_str.contains("成人") || name_str.contains("18") {
						content_rating = ContentRating::NSFW;
					}
				}
			}
		}

		let url = Some(format!("https://manhua.zaimanhua.com/details/{}", key));

		Manga {
			key,
			title: self.title.unwrap_or_default(),
			cover: self.cover,
			description: self.description,
			authors,
			tags,
			status,
			content_rating,
			viewer: match (self.direction, self.islong) {
				(Some(2), Some(1)) => Viewer::Webtoon,    // direction=2 + islong=1 = strip
				(Some(2), _) => Viewer::LeftToRight,     // direction=2 = LTR
				_ => Viewer::RightToLeft,                // direction=1 or null = RTL
			},
			url,
			..Default::default()
		}
	}

	pub fn into_chapters(self, manga_id: &str) -> Vec<Chapter> {
		let mut all_chapters = Vec::new();
		let comic_py = self.comic_py.clone().unwrap_or_default();

		if let Some(groups) = self.chapters {
			for group in groups {
				let group_title = group.title.clone().unwrap_or_default();
				for item in group.data {
					let chapter_id = item.chapter_id.to_string();
					let url = Some(format!(
						"https://manhua.zaimanhua.com/view/{}/{}/{}",
						comic_py, manga_id, chapter_id
					));

					all_chapters.push(Chapter {
						key: format!("{}/{}", manga_id, chapter_id),
						title: item.chapter_title,
						scanlators: Some(vec![group_title.clone()]),
						date_uploaded: item.updatetime,
						url,
						..Default::default()
					});
				}
			}
		}

		let total = all_chapters.len() as f32;
		for (idx, chapter) in all_chapters.iter_mut().enumerate() {
			chapter.chapter_number = Some(total - idx as f32);
		}

		all_chapters
	}
}

// === Subscribe List ===
// Response: data.subList[] with id, name, cover, authors

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
		let authors = item.authors.map(|a| vec![a]);

		Self {
			key,
			title: item.name.unwrap_or_default(),
			cover: item.cover,
			authors,
			content_rating: ContentRating::Safe,
			..Default::default()
		}
	}
}

// === Chapter Pages ===

#[derive(Deserialize)]
pub struct ChapterData {
	pub data: ChapterPageData,
}

#[derive(Deserialize)]
pub struct ChapterPageData {
	pub page_url: Option<Vec<String>>,
	pub page_url_hd: Option<Vec<String>>,
}

// === Helper Functions ===

fn parse_status(status_str: &str) -> MangaStatus {
	match status_str {
		s if s.contains("连载") => MangaStatus::Ongoing,
		s if s.contains("完结") => MangaStatus::Completed,
		s if s.contains("停更") || s.contains("暂停") => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

// === Convenience Functions ===

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

// === Recommend API (Home Banner) ===
#[derive(Deserialize)]
pub struct RecommendCategory {
	pub category_id: i64,
	#[allow(dead_code)]
	pub title: String,
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

// === Auth & User Info ===

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

#[derive(Deserialize, Clone)]
pub struct UserInfo {
	pub username: Option<String>,
	pub nickname: Option<String>,
	#[serde(rename = "user_level")]
	pub level: Option<i64>,
	pub is_sign: Option<bool>,
}
