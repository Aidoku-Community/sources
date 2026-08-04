use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct SearchResult {
	pub suggestions: Vec<Suggestion>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Suggestion {
	pub value: String,
	pub data: String,
	pub cover: Option<String>,
}
