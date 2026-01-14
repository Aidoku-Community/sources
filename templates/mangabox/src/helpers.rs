use crate::Params;
use aidoku::{
	FilterValue, MangaStatus,
	alloc::{String, Vec},
	prelude::*,
};

pub fn status_from_string(string: &str) -> MangaStatus {
	let string = string.trim();
	let status = string
		.rsplit_once(' ')
		.map(|(_, last)| last)
		.unwrap_or(string);
	match status {
		"Ongoing" => MangaStatus::Ongoing,
		"Completed" => MangaStatus::Completed,
		"Hiatus" => MangaStatus::Hiatus,
		"Cancelled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn strip_default_chapter_title(s: String) -> String {
	const PREFIX: &str = "Chapter ";

	s[PREFIX.len()..]
		.find(' ')
		.map(|space_pos| s[PREFIX.len() + space_pos..].into())
		.unwrap_or(s)
}

pub fn get_search_url(
	params: &Params,
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> String {
	if let Some(query) = query {
		// if there's a query, we can't filter
		return format!(
			"{}{}/{}?page={page}",
			params.base_url,
			params.search_path,
			encode(query, '_')
		);
	}

	enum SortOption {
		Newest,
		Latest,
		TopRead,
	}
	enum StatusOption {
		All,
		Completed,
		Ongoing,
	}
	let mut sort = SortOption::Newest;
	let mut status = StatusOption::All;
	let mut genre = String::from("all");

	for filter in filters {
		match filter {
			FilterValue::Sort { index, .. } => {
				sort = match index {
					0 => SortOption::Newest,
					1 => SortOption::Latest,
					2 => SortOption::TopRead,
					_ => SortOption::Newest,
				};
			}
			FilterValue::Select { id, value } => match id.as_str() {
				"status" => match value.as_str() {
					"Completed" => status = StatusOption::Completed,
					"Ongoing" => status = StatusOption::Ongoing,
					_ => {}
				},
				"genre" => {
					genre = encode(value, '-');
				}
				_ => {}
			},
			FilterValue::Text { value, .. } => {
				// author search
				return format!("{}/author/{}", params.base_url, encode(value, '-'));
			}
			_ => {}
		}
	}

	let url_filter = match sort {
		SortOption::Newest => 1,
		SortOption::Latest => 4,
		SortOption::TopRead => 7,
	} + match status {
		StatusOption::All => 0,
		StatusOption::Completed => 1,
		StatusOption::Ongoing => 2,
	};
	format!(
		"{}/genre/{genre}?filter={url_filter}&page={page}",
		params.base_url
	)
}

pub fn encode(string: String, separator: char) -> String {
	string
		.chars()
		.filter_map(|c| {
			if c.is_alphanumeric() {
				Some(c.to_ascii_lowercase())
			} else if [' ', '-', '_', '\'', '’'].contains(&c) {
				Some(separator)
			} else {
				None
			}
		})
		.collect()
}
