use aidoku::alloc::String;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ChapterData {
	pub s: String,
	pub ct: String,
}
