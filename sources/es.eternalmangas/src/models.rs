use aidoku::alloc::String;
use serde::Deserialize;
#[derive(Deserialize)]
pub struct LegacyPostResponse {
	pub post: Slug,
}
#[derive(Deserialize)]
pub struct LegacyChapterResponse {
	pub chapter: Slug,
}
#[derive(Deserialize)]
pub struct Slug {
	pub slug: String,
}
