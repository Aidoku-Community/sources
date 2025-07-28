use aidoku::{
	alloc::{format, string::String, vec::Vec},
	helpers::uri::QueryParameters,
	FilterValue,
};

pub fn parse_filters(query: Option<String>, filters: Vec<FilterValue>) -> String {
	let mut qs = QueryParameters::new();

	qs.push("s", Some(query.as_deref().unwrap_or("")));

	for filter in filters {
		match filter {
			FilterValue::Text { id, value } => {
				if !value.is_empty() {
					qs.push(&id, Some(&value));
				}
			}
			FilterValue::MultiSelect { id, included, .. } => {
				for value in included {
					qs.push(format!("{id}[]").as_str(), Some(&value));
				}
			}
			FilterValue::Select { id, value } => match id.as_str() {
				"adult" => {
					let adult = match value.as_str() {
						"No" => Some("0"),
						"Only" => Some("1"),
						_ => None,
					};
					if let Some(adult_value) = adult {
						qs.push("adult", Some(adult_value));
					}
				}
				"op" => {
					let op = match value.as_str() {
						"AND" => Some("1"),
						_ => None,
					};
					if let Some(op_value) = op {
						qs.push("op", Some(op_value));
					}
				}
				_ => {}
			},
			FilterValue::Sort { index, .. } => {
				let sort_value = match index {
					1 => Some("latest"),
					2 => Some("alphabet"),
					3 => Some("rating"),
					4 => Some("trending"),
					5 => Some("views"),
					6 => Some("new-manga"),
					_ => None,
				};

				if let Some(value) = sort_value {
					qs.push("m_orderby", Some(value));
				}
			}
			_ => {}
		}
	}

	format!("{qs}")
}
