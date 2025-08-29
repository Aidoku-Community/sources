use aidoku::alloc::String;
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PageListResponse {
	pub status: bool,
	pub msg: Option<String>,
	pub html: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChapterData {
	pub s: String,
	pub ct: String,
}
