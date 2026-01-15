use aidoku::{
	Chapter,
	alloc::{string::String, vec::Vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApiResponse<T> {
	// pub success: bool,
	pub data: T,
}

#[derive(Deserialize)]
pub struct ChaptersResponse {
	pub chapters: Vec<ChapterData>,
	pub pagination: Pagination,
}

#[derive(Deserialize)]
pub struct Pagination {
	pub has_more: bool,
}

#[derive(Deserialize)]
pub struct ChapterData {
	pub chapter_name: String,
	pub chapter_slug: String,
	pub chapter_num: f32,
	pub updated_at: String,
	// pub view: i32,
}

impl From<ChapterData> for Chapter {
	fn from(value: ChapterData) -> Self {
		Chapter {
			key: value.chapter_slug,
			title: Some(
				value
					.chapter_name
					.trim_start_matches(&format!("Chapter {}", value.chapter_num))
					.into(),
			),
			chapter_number: Some(value.chapter_num),
			date_uploaded: parse_date(&value.updated_at, "yyyy-MM-dd'T'HH:mm:ss.SSSSSS'Z'"),
			..Default::default()
		}
	}
}
