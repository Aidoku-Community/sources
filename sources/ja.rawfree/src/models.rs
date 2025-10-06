use aidoku::alloc::string::String;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AjaxResponse {
	pub mes: String,
	// pub going: i32,
	pub img_index: i32,
	pub chapter_id: Option<String>,
}
