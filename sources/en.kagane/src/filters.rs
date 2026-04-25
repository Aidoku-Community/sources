use aidoku::{
	FilterValue,
	alloc::{String, Vec, vec},
};
use serde_json::{Map, Value};

const SORT_FIELDS: &[&str] = &[
	"",
	"total_views",
	"avg_views",
	"avg_views_today",
	"avg_views_week",
	"avg_views_month",
	"updated_at",
	"series_name",
	"books_count",
	"created_at",
];

pub fn build_search_body(
	query: Option<&str>,
	filters: &[FilterValue],
	default_ratings: &[&str],
) -> (String, String) {
	let mut sort_param = String::new();
	let mut content_ratings: Vec<String> =
		default_ratings.iter().map(|s| String::from(*s)).collect();
	let mut format_values: Vec<String> = Vec::new();
	let mut status_values: Vec<String> = Vec::new();

	for filter in filters {
		match filter {
			FilterValue::Sort {
				index, ascending, ..
			} => {
				let idx = (*index as usize).min(SORT_FIELDS.len() - 1);
				let field = SORT_FIELDS[idx];
				if !field.is_empty() {
					sort_param = if *ascending {
						String::from(field)
					} else {
						let mut s = String::from(field);
						s.push_str(",desc");
						s
					};
				}
			}
			FilterValue::MultiSelect { id, included, .. } => match id.as_str() {
				"content_rating" if !included.is_empty() => {
					content_ratings = included.iter().cloned().collect();
				}
				"format" if !included.is_empty() => {
					format_values = included.iter().cloned().collect();
				}
				"upload_status" if !included.is_empty() => {
					status_values = included.iter().cloned().collect();
				}
				_ => {}
			},
			_ => {}
		}
	}

	let mut map = Map::new();

	if let Some(q) = query.filter(|s| !s.is_empty()) {
		map.insert("title".into(), Value::String(String::from(q)));
	}

	map.insert(
		"source_type".into(),
		Value::Array(vec![
			Value::String("Official".into()),
			Value::String("Unofficial".into()),
			Value::String("Mixed".into()),
		]),
	);

	if !content_ratings.is_empty() {
		map.insert(
			"content_rating".into(),
			Value::Array(content_ratings.into_iter().map(Value::String).collect()),
		);
	}

	if !format_values.is_empty() {
		map.insert(
			"format".into(),
			Value::Array(format_values.into_iter().map(Value::String).collect()),
		);
	}

	if !status_values.is_empty() {
		map.insert(
			"upload_status".into(),
			Value::Array(status_values.into_iter().map(Value::String).collect()),
		);
	}

	let body = serde_json::to_string(&Value::Object(map)).unwrap_or_else(|_| "{}".into());
	(body, sort_param)
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::alloc::vec;
	use aidoku::FilterValue;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_build_body_empty_query() {
		let (body, sort) = build_search_body(None, &[], &["Safe", "Suggestive"]);
		assert!(body.contains("source_type"));
		assert!(body.contains("Safe"));
		assert!(sort.is_empty());
	}

	#[aidoku_test]
	fn test_build_body_with_query() {
		let (body, _) = build_search_body(Some("attack"), &[], &["Safe"]);
		assert!(body.contains("\"title\""));
		assert!(body.contains("attack"));
	}

	#[aidoku_test]
	fn test_build_body_sort_filter() {
		let filters = vec![FilterValue::Sort {
			id: "sort".into(),
			index: 1,
			ascending: false,
		}];
		let (_, sort) = build_search_body(None, &filters, &["Safe"]);
		assert_eq!(sort, "total_views,desc");
	}

	#[aidoku_test]
	fn test_build_body_sort_ascending() {
		let filters = vec![FilterValue::Sort {
			id: "sort".into(),
			index: 6,
			ascending: true,
		}];
		let (_, sort) = build_search_body(None, &filters, &["Safe"]);
		assert_eq!(sort, "updated_at");
	}

	#[aidoku_test]
	fn test_build_body_format_filter() {
		let filters = vec![FilterValue::MultiSelect {
			id: "format".into(),
			included: vec!["Manga".into(), "Manhwa".into()],
			excluded: vec![],
		}];
		let (body, _) = build_search_body(None, &filters, &["Safe"]);
		assert!(body.contains("\"format\""));
		assert!(body.contains("Manga"));
	}

	#[aidoku_test]
	fn test_build_body_content_rating_from_filter() {
		let filters = vec![FilterValue::MultiSelect {
			id: "content_rating".into(),
			included: vec!["Safe".into()],
			excluded: vec![],
		}];
		let (body, _) = build_search_body(None, &filters, &["Safe", "Suggestive"]);
		assert!(body.contains("content_rating"));
		assert!(body.contains("Safe"));
	}
}
