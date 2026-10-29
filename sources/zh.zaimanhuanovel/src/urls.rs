use aidoku::alloc::{String, format};
use zaimanhua_template::V4_API_URL;

pub use zaimanhua_template::net::urls::*;

pub fn tags() -> String {
	format!("{V4_API_URL}/novel/rank/tags")
}

pub fn chapters(id: &str) -> String {
	format!("{V4_API_URL}/novel/chapter/{id}")
}

pub fn text(volume_id: &str, chapter_id: &str) -> String {
	format!("{V4_API_URL}/novel/download/chapter?volumeId={volume_id}&chapterId={chapter_id}")
}

pub fn rank(rank_type: &str, page: i32) -> String {
	format!("{V4_API_URL}/novel/rank/list?rankType={rank_type}&page={page}")
}

pub fn recommend() -> String {
	format!("{V4_API_URL}/novel/recommend")
}

pub fn hot_keywords() -> String {
	format!("{V4_API_URL}/search/hot/keywords?type=novel")
}
