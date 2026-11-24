use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString},
	imports::std::parse_date_with_options,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Response<T> {
	pub data: T,
}

/// `/series` catalog page.
#[derive(Deserialize)]
pub struct SeriesList {
	pub data: Vec<Series>,
	pub meta: SeriesListMeta,
}

#[derive(Deserialize)]
pub struct SeriesListMeta {
	pub page: i32,
	pub total_pages: i32,
}

#[derive(Deserialize)]
pub struct Series {
	pub slug: String,
	pub title: String,
	pub author: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub status: Option<String>,
	pub description_text: Option<String>,
	pub cover_url: Option<String>,
	pub url: Option<String>,
	#[serde(default)]
	pub categories: Vec<Category>,
}

#[derive(Deserialize)]
pub struct Category {
	pub name: String,
}

/// `/series/{slug}/chapters` page, newest first.
#[derive(Deserialize)]
pub struct ChapterList {
	pub data: Vec<ChapterItem>,
	pub meta: ChapterListMeta,
}

#[derive(Deserialize)]
pub struct ChapterListMeta {
	/// Offset of the next page.
	pub offset: i32,
	pub has_more: bool,
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub slug: String,
	pub display_number: Option<String>,
	pub sort_number: Option<f32>,
	pub title: Option<String>,
	pub created_at: Option<String>,
	pub url: Option<String>,
}

/// `/series/{slug}/chapters/{chapter}` reader data.
#[derive(Deserialize)]
pub struct ChapterPages {
	pub pages: Vec<PageItem>,
}

#[derive(Deserialize)]
pub struct PageItem {
	pub image_url: String,
}

impl From<SeriesList> for MangaPageResult {
	fn from(list: SeriesList) -> Self {
		MangaPageResult {
			has_next_page: list.meta.page < list.meta.total_pages,
			entries: list.data.into_iter().map(Into::into).collect(),
		}
	}
}

impl From<Series> for Manga {
	fn from(series: Series) -> Self {
		let viewer = match series.kind.as_deref() {
			Some("manga") => Viewer::RightToLeft,
			_ => Viewer::Webtoon,
		};
		let status = match series.status.as_deref() {
			Some("ongoing") => MangaStatus::Ongoing,
			Some("completed") => MangaStatus::Completed,
			Some("hiatus") => MangaStatus::Hiatus,
			Some("cancelled" | "dropped") => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		};
		// "Ecchi" is the only suggestive genre on the site
		let content_rating = if series.categories.iter().any(|c| c.name == "Ecchi") {
			ContentRating::Suggestive
		} else {
			ContentRating::Safe
		};
		let authors = series
			.author
			.map(|a| a.split(", ").map(ToString::to_string).collect());
		let tags = Some(series.categories.into_iter().map(|c| c.name).collect());
		Manga {
			key: series.slug,
			title: series.title,
			cover: series.cover_url,
			authors,
			description: series.description_text,
			url: series.url,
			tags,
			status,
			content_rating,
			viewer,
			..Default::default()
		}
	}
}

impl From<ChapterItem> for Chapter {
	fn from(item: ChapterItem) -> Self {
		// titles usually just repeat the number ("Bölüm 366"); only keep real ones
		let title = item
			.title
			.filter(|t| t.strip_prefix("Bölüm ") != item.display_number.as_deref());
		let date_uploaded = item.created_at.and_then(|date| {
			parse_date_with_options(
				date,
				"yyyy-MM-dd HH:mm:ss",
				"en_US_POSIX",
				"Europe/Istanbul",
			)
		});
		Chapter {
			key: item.slug,
			title,
			chapter_number: item.sort_number,
			date_uploaded,
			url: item.url,
			..Default::default()
		}
	}
}
