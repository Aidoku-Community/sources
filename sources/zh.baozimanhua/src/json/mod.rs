use crate::BASE_URL;
use aidoku::{
	Manga, MangaPageResult,
	alloc::{String, Vec, string::ToString as _, vec},
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
	author: String,
	type_names: Vec<String>,
	region_name: Option<String>,
	region: Option<String>,
}

impl From<ApiResponse> for MangaPageResult {
	fn from(response: ApiResponse) -> Self {
		let entries = response
			.items
			.into_iter()
			.map(|item| {
				let key = item.comic_id.clone();
				let cover = Some(format!(
					"https://static-tw.baozimh.com/cover/{}",
					item.topic_img
				));
				let title = item.name;

				// Deduplicate authors
				let mut artists: Vec<String> = item
					.author
					.split(',')
					.map(|s| s.trim().to_string())
					.filter(|s| !s.is_empty())
					.collect();
				artists.dedup();
				let artist_str = artists.join(", ");

				// Filter out ASCII-only genres and add region
				let mut categories: Vec<String> = item
					.type_names
					.into_iter()
					.filter(|g| !g.is_ascii())
					.collect();

				if let Some(region) = item.region_name.or(item.region) {
					categories.insert(0, region);
				}

				Manga {
					key: key.clone(),
					cover,
					title,
					authors: Some(vec![artist_str]),
					url: Some(format!("{}/comic/{}", BASE_URL, key)),
					tags: Some(categories),
					..Default::default()
				}
			})
			.collect();

		let has_next_page = response.next.is_some();

		MangaPageResult {
			entries,
			has_next_page,
		}
	}
}
