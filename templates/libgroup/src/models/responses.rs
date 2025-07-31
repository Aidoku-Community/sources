use aidoku::alloc::Vec;
use serde::Deserialize;

use crate::models::common::LibGroupMeta;

use super::{
	chapter::{LibGroupChapter, LibGroupChapterListItem},
	constants::LibGroupConstantsData,
	manga::LibGroupManga,
};

#[derive(Default, Deserialize, Debug, Clone)]
pub struct MangaListResponse {
	pub data: Vec<LibGroupManga>,
	pub meta: LibGroupMeta,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct MangaDetailResponse {
	pub data: LibGroupManga,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ChaptersResponse {
	pub data: Vec<LibGroupChapterListItem>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ChapterResponse {
	pub data: LibGroupChapter,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ConstantsResponse {
	pub data: LibGroupConstantsData,
}
