use crate::helpers::parse_chapter_number;
use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::html::Html,
	prelude::*,
};
use serde::Deserialize;

use crate::BASE_URL;

#[derive(Deserialize)]
pub struct ApiResponse<T> {
	pub data: Data<T>,
}

#[derive(Deserialize)]
pub struct Data<T> {
	pub result: T,
	pub extra: Option<Extra>,
}

#[derive(Deserialize)]
pub struct Extra {
	pub has_next: Option<bool>,
}

impl From<ApiResponse<Vec<NicoManga>>> for MangaPageResult {
	fn from(value: ApiResponse<Vec<NicoManga>>) -> Self {
		MangaPageResult {
			entries: value.data.result.into_iter().map(Into::into).collect(),
			has_next_page: value
				.data
				.extra
				.and_then(|extra| extra.has_next)
				.unwrap_or_default(),
		}
	}
}

impl From<ApiResponse<NicoManga>> for Manga {
	fn from(value: ApiResponse<NicoManga>) -> Self {
		value.data.result.into()
	}
}

#[derive(Deserialize)]
pub struct NicoManga {
	pub id: i32,
	pub meta: NicoMangaMetadata,
}

#[derive(Deserialize)]
pub struct NicoMangaMetadata {
	// pub player_type: String, // scroll, ?
	pub title: String,
	pub description: String,
	pub serial_status: String,
	pub square_image_url: Option<String>,
	pub icon_url: Option<String>,
	pub category: Option<String>, // shonen, joshi, seinen, shojo, fan, yonkoma, other
	pub share_url: String,
	pub authors: Option<Vec<NicoAuthor>>,
}

#[derive(Deserialize)]
pub struct NicoAuthor {
	pub name: String,
}

impl From<NicoManga> for Manga {
	fn from(value: NicoManga) -> Self {
		Manga {
			key: value.id.to_string(),
			title: value.meta.title,
			cover: value.meta.square_image_url.or(value.meta.icon_url),
			authors: value
				.meta
				.authors
				.map(|authors| authors.into_iter().map(|author| author.name).collect()),
			description: Html::parse_fragment(&value.meta.description)
				.ok()
				.and_then(|html| html.select_first("body"))
				.and_then(|body| body.text())
				.or(Some(value.meta.description)),
			url: Some(value.meta.share_url),
			tags: value.meta.category.map(|category| {
				vec![String::from(match category.as_str() {
					"shonen" => "少年マンガ",
					"shojo" => "少女マンガ",
					"seinen" => "青年マンガ",
					"josei" => "女性マンガ",
					"fan" => "ファンコミック",
					"yonkoma" => "4コママンガ",
					_ => "その他マンガ",
				})]
			}),
			status: match value.meta.serial_status.as_str() {
				"serial" => MangaStatus::Ongoing,
				"concluded" => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			},
			viewer: Viewer::RightToLeft,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct NicoChapter {
	pub id: i32,
	pub meta: NicoChapterMetadata,
	pub own_status: NicoChapterOwnership,
}

#[derive(Deserialize)]
pub struct NicoChapterMetadata {
	pub title: String,
	// pub number: i32,
	pub thumbnail_url: Option<String>,
	pub created_at: i64,
}

#[derive(Deserialize)]
pub struct NicoChapterOwnership {
	pub sell_status: String,
}

impl From<NicoChapter> for Chapter {
	fn from(value: NicoChapter) -> Self {
		let result = parse_chapter_number(&value.meta.title);
		let chapter_number = result.as_ref().map(|r| r.0);
		Chapter {
			key: value.id.to_string(),
			title: Some(result.map(|r| r.1).unwrap_or(value.meta.title)),
			chapter_number,
			date_uploaded: Some(value.meta.created_at),
			url: Some(format!("{BASE_URL}/watch/mg{}", value.id)),
			thumbnail: value.meta.thumbnail_url,
			locked: value.own_status.sell_status == "selling",
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct NicoFrame {
	pub id: i32,
	pub meta: NicoFrameMetadata,
}

#[derive(Deserialize)]
pub struct NicoFrameMetadata {
	pub source_url: String,
	pub drm_hash: Option<String>,
}

impl From<NicoFrame> for Page {
	fn from(value: NicoFrame) -> Self {
		let mut context = PageContext::new();
		if let Some(drm_hash) = value.meta.drm_hash {
			context.insert("drm_hash".into(), drm_hash);
		}
		Page {
			content: PageContent::url_context(value.meta.source_url, context),
			..Default::default()
		}
	}
}
