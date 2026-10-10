use aidoku::{
	Filter, MultiSelectFilter,
	alloc::{String, Vec},
};
use serde::Deserialize;

use crate::helpers::{api_post, fetch_json};

#[derive(Deserialize)]
struct BigTagResp {
	data: Option<Vec<BigTag>>,
}

#[derive(Deserialize)]
struct BigTag {
	tag: String,
	search: Vec<String>,
}

pub fn build_tag_filter(adult_mode: &str) -> Filter {
	let mut options: Vec<String> = Vec::new();
	let mut ids: Vec<String> = Vec::new();

	if let Ok(tags) = fetch_bigtaglist(adult_mode) {
		for t in tags {
			// search term may differ from the display tag name
			let search_term: String = t.search.into_iter().next().unwrap_or_else(|| t.tag.clone());
			if ids.contains(&search_term) {
				continue;
			}
			options.push(t.tag);
			ids.push(search_term);
		}
	}

	MultiSelectFilter {
		id: "tag".into(),
		title: Some("標籤".into()),
		is_genre: true,
		can_exclude: false,
		options: options.into_iter().map(|s| s.into()).collect(),
		ids: Some(ids.into_iter().map(|s| s.into()).collect()),
		..Default::default()
	}
	.into()
}

fn fetch_bigtaglist(adult_mode: &str) -> aidoku::Result<Vec<BigTag>> {
	let resp: BigTagResp = fetch_json(|| api_post("/api/bigtaglist", "", adult_mode))?;
	Ok(resp.data.unwrap_or_default())
}
