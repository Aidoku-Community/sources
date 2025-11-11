use aidoku::{
	ContentRating, Manga, MangaPageResult, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
	serde::Deserialize,
};

use chrono::DateTime;

#[derive(Deserialize)]
pub struct ComicItem {
	pub _id: String,
	pub title: String,
	pub author: String,
	pub description: Option<String>,
	pub thumb: Thumb,
	pub categories: Vec<String>,
	pub tags: Option<Vec<String>>,
	pub finished: bool,
	#[serde(rename = "pagesCount")]
	pub pages_count: Option<i32>,
	#[serde(rename = "likesCount")]
	pub likes_count: Option<i32>,
	#[serde(rename = "totalLikes")]
	pub total_likes: Option<i32>,
	#[serde(rename = "chineseTeam")]
	pub chinese_team: Option<String>,
	pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct Thumb {
	#[serde(rename = "fileServer")]
	pub file_server: String,
	pub path: String,
}

#[derive(Deserialize)]
pub struct ComicResponse {
	pub data: ComicData,
}

#[derive(Deserialize)]
pub struct ComicData {
	pub comic: ComicItem,
}

#[derive(Deserialize)]
pub struct ExploreResponse {
	pub data: ExploreData,
}

#[derive(Deserialize)]
pub struct ExploreData {
	pub comics: ComicsData,
}

#[derive(Deserialize)]
pub struct ComicsData {
	pub docs: Vec<ComicItem>,
	pub page: i32,
	pub pages: i32,
}

#[derive(Deserialize)]
pub struct RankResponse {
	pub data: RankData,
}

#[derive(Deserialize)]
pub struct RankData {
	pub comics: Vec<ComicItem>,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub _id: String,
	pub order: i32,
	pub title: String,
	pub updated_at: String,
}

#[derive(Deserialize)]
pub struct ChapterResponse {
	pub data: ChapterData,
}

#[derive(Deserialize)]
pub struct ChapterData {
	pub eps: EpsData,
}

#[derive(Deserialize)]
pub struct EpsData {
	pub docs: Vec<ChapterItem>,
	pub pages: i32,
}

#[derive(Deserialize)]
pub struct PageItem {
	pub media: Media,
}

#[derive(Deserialize)]
pub struct Media {
	#[serde(rename = "fileServer")]
	pub file_server: String,
	pub path: String,
}

#[derive(Deserialize)]
pub struct PageResponse {
	pub data: PageData,
}

#[derive(Deserialize)]
pub struct PageData {
	pub pages: PagesData,
}

#[derive(Deserialize)]
pub struct PagesData {
	pub docs: Vec<PageItem>,
	pub pages: i32,
	pub limit: i32,
}

impl From<ComicItem> for Manga {
	fn from(item: ComicItem) -> Self {
		let cover = format!("{}/static/{}", item.thumb.file_server, item.thumb.path);
		let author = item
			.author
			.split("&")
			.map(|a| a.trim().to_string())
			.collect::<Vec<String>>()
			.join(", ");
		let status = if item.finished {
			MangaStatus::Completed
		} else {
			MangaStatus::Ongoing
		};
		let url = format!("https://manhuabika.com/pcomicview/?cid={}", item._id);

		let mut all_tags = Vec::new();
		if let Some(tags) = item.tags {
			all_tags.extend(tags);
		}
		let viewer = if item.categories.iter().any(|tag| tag.contains("WEBTOON")) {
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};
		all_tags.extend(item.categories);

		let pages_text = item.pages_count.map(|count| format!("页数：{}P", count));
		let likes_text = item
			.total_likes
			.or(item.likes_count)
			.map(|count| format!("{} likes", count));

		let mut desc_parts = Vec::new();

		if let Some(text) = likes_text {
			desc_parts.push(text);
		}

		if let Some(text) = pages_text {
			desc_parts.push(text);
		}

		if let Some(desc) = item.description
			&& !desc.trim().is_empty()
		{
			desc_parts.push(format!("简介：{}", desc));
		}

		let description = if desc_parts.is_empty() {
			None
		} else {
			Some(desc_parts.join("  \n"))
		};

		Manga {
			key: item._id,
			cover: Some(cover),
			title: item.title,
			authors: Some(vec![author]),
			description,
			url: Some(url),
			tags: Some(all_tags),
			status,
			content_rating: ContentRating::NSFW,
			viewer,
			..Default::default()
		}
	}
}

impl From<ExploreData> for MangaPageResult {
	fn from(data: ExploreData) -> Self {
		let blocklist = crate::settings::get_blocklist();
		let entries = data
			.comics
			.docs
			.into_iter()
			.map(Into::into)
			.filter(|manga: &Manga| !hit_blocklist(manga, &blocklist))
			.collect();
		let has_next_page = data.comics.page < data.comics.pages;

		MangaPageResult {
			entries,
			has_next_page,
		}
	}
}

fn hit_blocklist(manga: &Manga, blocklist: &[String]) -> bool {
	if blocklist.is_empty() {
		return false;
	}

	if let Some(ref categories) = manga.tags {
		categories.iter().any(|tag| blocklist.contains(tag))
	} else {
		false
	}
}

impl From<RankData> for MangaPageResult {
	fn from(data: RankData) -> Self {
		let blocklist = crate::settings::get_blocklist();
		let entries = data
			.comics
			.into_iter()
			.map(Into::into)
			.filter(|manga: &Manga| !hit_blocklist(manga, &blocklist))
			.collect();

		MangaPageResult {
			entries,
			has_next_page: false,
		}
	}
}

impl From<ChapterItem> for aidoku::Chapter {
	fn from(item: ChapterItem) -> Self {
		aidoku::Chapter {
			key: item.order.to_string(),
			title: Some(item.title),
			chapter_number: Some(item.order as f32),
			date_uploaded: DateTime::parse_from_rfc3339(&item.updated_at)
				.ok()
				.map(|d| d.timestamp()),
			..Default::default()
		}
	}
}

impl From<PageItem> for aidoku::Page {
	fn from(item: PageItem) -> Self {
		let url = format!("{}/static/{}", item.media.file_server, item.media.path);
		aidoku::Page {
			content: aidoku::PageContent::url(url),
			..Default::default()
		}
	}
}
