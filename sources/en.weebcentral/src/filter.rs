use crate::helper;
use crate::model::SortOptions;
use aidoku::{
	alloc::{borrow::ToOwned, String, Vec},
	helpers::uri::QueryParameters,
	prelude::*,
	FilterValue,
};

pub fn get_filters(query: Option<String>, filters: Vec<FilterValue>) -> String {
	let mut qs = QueryParameters::new();

	if let Some(query) = query {
		let cleaned = helper::remove_special_chars(query).trim().to_owned();
		if !cleaned.is_empty() {
			qs.push("text", Some(&cleaned));
		}
	}

	let mut extra_params = String::new();

	for filter in filters {
		match filter {
			FilterValue::Text { ref value, .. } => {
				if !value.is_empty() {
					qs.push("author", Some(value));
				}
			}
			FilterValue::Sort {
				index, ascending, ..
			} => {
				let option: &str = SortOptions::from(index).into();
				qs.push("sort", Some(option));
				qs.push(
					"order",
					Some(if ascending { "Ascending" } else { "Descending" }),
				);
			}
			FilterValue::MultiSelect {
				ref id,
				ref included,
				ref excluded,
			} => {
				if id == "genre" {
					for tag in included {
						qs.push("included_tag", Some(tag));
					}
					for tag in excluded {
						qs.push("excluded_tag", Some(tag));
					}
				} else {
					for val in included {
						extra_params.push_str(val);
					}
				}
			}
			FilterValue::Check { value, .. } => qs.push(
				"official",
				Some(match value {
					0 => "False",
					1 => "True",
					_ => "Any",
				}),
			),
			_ => {}
		}
	}

	format!("{qs}{extra_params}")
}
