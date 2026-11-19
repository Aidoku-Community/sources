use crate::{
	helpers::{number_list, site_headers},
	settings,
};
use aidoku::{
	FilterValue, Result,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use serde::Serialize;

const SORTS: [&str; 4] = ["post", "release", "title", "popular"];

#[derive(Serialize)]
pub struct SearchParams {
	pub page: i32,
	pub search: String,
	pub years: String,
	pub genres: String,
	pub types: String,
	pub statuses: String,
	pub sort: String,
	#[serde(rename = "genreMatchMode")]
	pub genre_match_mode: String,
	#[serde(skip)]
	pub excluded_tags: Vec<String>,
	#[serde(skip)]
	pub excluded_types: Vec<String>,
}

impl SearchParams {
	pub fn new(query: Option<String>, page: i32, filters: Vec<FilterValue>) -> Result<Self> {
		let mut search = Self {
			page,
			search: query.unwrap_or_default(),
			years: "[]".into(),
			genres: "[]".into(),
			types: "[]".into(),
			statuses: "[]".into(),
			sort: "post_desc".into(),
			genre_match_mode: "any".into(),
			excluded_tags: settings::excluded_tags(),
			excluded_types: settings::excluded_types(),
		};
		// A tag or type the search includes overrides its exclusion in settings
		let mut overrides = Vec::new();
		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					if let Some(sort) = SORTS.get(index as usize) {
						search.sort = format!("{sort}_{}", if ascending { "asc" } else { "desc" });
					}
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"genre" => {
						search.genres = number_list(&included)?;
						search.excluded_tags.extend(excluded);
						overrides.extend(included);
					}
					"type" => {
						search.types = serde_json::to_string(&included)?;
						search.excluded_types.extend(excluded);
						overrides.extend(included);
					}
					_ => {}
				},
				FilterValue::Select { id, value } => match id.as_str() {
					"status" => search.statuses = serde_json::to_string(&[value])?,
					"year" => search.years = number_list(&[value])?,
					"genreMatchMode" => search.genre_match_mode = value,
					_ => {}
				},
				_ => {}
			}
		}
		search.excluded_tags.retain(|id| !overrides.contains(id));
		search
			.excluded_types
			.retain(|kind| !overrides.contains(kind));
		Ok(search)
	}

	pub fn request(&self, base_url: &str) -> Result<Request> {
		Ok(site_headers(
			Request::post(format!("{base_url}/wp-json/manga/v1/load"))?,
			base_url,
		)
		.header("Content-Type", "application/json")
		.body(serde_json::to_vec(self)?))
	}
}
