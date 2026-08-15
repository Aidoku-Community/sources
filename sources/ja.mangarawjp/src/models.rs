use aidoku::alloc::{string::String, vec::Vec};
use serde::Deserialize;

// Response of /api/v1/get/c: `c` is the page shuffling key, `e` the image paths
// ("/public/key/?id=...").
#[derive(Deserialize)]
pub struct ChapterApiResponse {
	#[serde(default)]
	pub c: String,
	#[serde(default)]
	pub e: Vec<String>,
}
