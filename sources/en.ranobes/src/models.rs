use aidoku::alloc::String;
use serde::Deserialize;

/// The `window.__DATA__` blob embedded in a `<script>` tag on
/// `/chapters/{id}/` (and each `/chapters/{id}/page/{n}/`).
///
/// Confirmed against a real response. Extra fields present in the real
/// blob (`book_title`, `book_id`, `count_all`, `cstart`, `limit`, `search`,
/// etc.) are ignored — serde ignores unknown fields by default.
#[derive(Deserialize)]
pub struct ChapterListData {
	pub pages_count: i32,
	pub chapters: Vec<ChapterEntry>,
}

#[derive(Deserialize)]
pub struct ChapterEntry {
	pub title: String,
	pub link: String,
	pub date: String,
}
