use aidoku::alloc::String;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AjaxResponse {
	pub data: AjaxData,
}

#[derive(Deserialize)]
pub struct AjaxData {
	pub html: String,
}
