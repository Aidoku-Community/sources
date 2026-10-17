use crate::base_url;
use aidoku::{
	Manga, MangaPageResult,
	alloc::{String, Vec},
	prelude::*,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ApiResponse {
	items: Vec<MangaItem>,
	next: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct MangaItem {
	comic_id: String,
	topic_img: String,
	name: String,
}

impl From<ApiResponse> for MangaPageResult {
	fn from(response: ApiResponse) -> Self {
		let entries = response
			.items
			.into_iter()
			.map(|item| Manga {
				cover: Some(format!(
					"https://static-tw.baozimh.com/cover/{}",
					item.topic_img
				)),
				title: item.name,
				url: Some(format!("{}/comic/{}", base_url(), item.comic_id)),
				key: item.comic_id,
				..Default::default()
			})
			.collect();

		let has_next_page = response.next.is_some();

		MangaPageResult {
			entries,
			has_next_page,
		}
	}
}
