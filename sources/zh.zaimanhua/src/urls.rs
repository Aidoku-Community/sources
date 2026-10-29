use aidoku::{
	alloc::{String, format},
	helpers::uri::encode_uri_component,
};
use zaimanhua_template::{PAGE_SIZE, V4_API_URL};

pub use zaimanhua_template::net::urls::*;

pub fn classify() -> String {
	format!("{V4_API_URL}/comic/filter/classify")
}

pub fn author(tag_id: i64, page: i32) -> String {
	format!("{V4_API_URL}/comic/list_by_author?tag_id={tag_id}&page={page}&size={PAGE_SIZE}")
}

pub fn rank(by_time: &str, page: i32) -> String {
	format!("{V4_API_URL}/comic/rank/list?by_time={by_time}&page={page}")
}

pub fn detail_by_pinyin(pinyin: &str) -> String {
	let pinyin = encode_uri_component(pinyin);
	format!("{V4_API_URL}/comic/detail/by_pinyin?pinyin={pinyin}")
}

pub fn chapter(chapter_key: &str) -> String {
	format!("{V4_API_URL}/comic/chapter/{chapter_key}")
}

pub fn viewpoints(comic_id: &str, chapter_id: &str) -> String {
	format!("{V4_API_URL}/viewpoint/list?comicId={comic_id}&chapterId={chapter_id}")
}

pub fn recommend() -> String {
	format!("{V4_API_URL}/comic/recommend/list")
}
