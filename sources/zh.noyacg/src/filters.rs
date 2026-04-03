use aidoku::{
	Filter, MultiSelectFilter,
	alloc::{String, Vec},
	prelude::format,
};
use serde::Deserialize;

use crate::helpers::{build_form_body, get_base_url, post_with_form};

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
	let base_url = get_base_url();
	let body = build_form_body(&[]);
	let resp: BigTagResp = post_with_form(
		&format!("{base_url}/api/bigtaglist"),
		&body,
		&format!("{base_url}/"),
		adult_mode,
	)?
	.json_owned()?;
	Ok(resp.data.unwrap_or_default())
}
