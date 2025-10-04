use aidoku::alloc::{String, Vec};
use serde::Deserialize;
use serde_json::Value;

#[derive(Default, Deserialize, Debug, Clone)]
pub struct AjaxResponse<T> {
	pub result: T,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct AjaxRead {
	pub html: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct AjaxPageList {
	pub images: Vec<Vec<Value>>,
}
