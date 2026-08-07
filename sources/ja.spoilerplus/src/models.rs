use aidoku::alloc::{string::String, vec::Vec};
use serde::Deserialize;

/// Response structure for /api/v1/get/c
#[derive(Deserialize)]
pub struct ChapterApiResponse {
	/// Page shuffling key
	#[serde(default)]
	pub c: String,
	/// List of image paths ("/public/key/?id=..." format)
	#[serde(default)]
	pub e: Vec<String>,
}
