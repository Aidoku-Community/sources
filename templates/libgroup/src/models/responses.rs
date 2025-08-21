use aidoku::alloc::{String, Vec};
use serde::{Deserialize, Serialize};

use crate::models::{common::LibGroupMeta, manga::LibGroupCoverItem, user::LibGroupUser};

use super::{
	chapter::{LibGroupChapter, LibGroupChapterListItem},
	constants::LibGroupConstantsData,
	manga::LibGroupManga,
};

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct UserResponse {
	pub data: LibGroupUser,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct MangaListResponse {
	pub data: Vec<LibGroupManga>,
	pub meta: LibGroupMeta,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct MangaDetailResponse {
	pub data: LibGroupManga,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ChaptersResponse {
	pub data: Vec<LibGroupChapterListItem>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ChapterResponse {
	pub data: LibGroupChapter,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct MangaCoversResponse {
	pub data: Vec<LibGroupCoverItem>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ConstantsResponse {
	pub data: LibGroupConstantsData,
}

#[derive(Deserialize, Serialize)]
pub struct TokenResponse {
	pub access_token: Option<String>,
	pub refresh_token: Option<String>,
	pub expires_in: Option<i64>,
}
