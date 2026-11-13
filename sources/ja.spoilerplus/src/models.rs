use aidoku::alloc::{string::String, vec::Vec};
use serde::Deserialize;

// Response of POST /api/v1/get/c
#[derive(Deserialize)]
pub struct ChapterApiResponse {
	// whether chapter is scrambled
	#[serde(default)]
	pub b: bool,
	// page shuffling key
	#[serde(default)]
	pub c: String,
	// image paths
	#[serde(default)]
	pub e: Vec<String>,
}
