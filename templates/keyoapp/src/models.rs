use aidoku::alloc::String;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchResponse {
	pub html: String,
	pub has_more: bool,
}

#[derive(Deserialize)]
pub struct ChaptersResponse {
	pub rows_html: String,
}
