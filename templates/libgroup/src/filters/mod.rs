use aidoku::{
	FilterValue,
	alloc::{String, Vec, string::ToString},
};

pub struct FilterProcessor;

impl FilterProcessor {
	pub const fn new() -> Self {
		Self
	}

	pub fn process_filters(&self, filters: Vec<FilterValue>) -> Vec<(&'static str, String)> {
		filters
			.into_iter()
			.flat_map(|filter| self.process_single_filter(filter))
			.collect()
	}

	fn process_single_filter(&self, filter: FilterValue) -> Vec<(&'static str, String)> {
		let mut params = Vec::new();

		match filter {
			FilterValue::Text { id, value } => {
				self.process_text_filter(&mut params, &id, &value);
			}
			FilterValue::Sort {
				id,
				index,
				ascending,
			} => {
				self.process_sort_filter(&mut params, &id, index, ascending);
			}
			FilterValue::Check { id, value } => {
				self.process_check_filter(&mut params, &id, value);
			}
			FilterValue::Select { .. } => {}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} => {
				self.process_multiselect_filter(&mut params, &id, &included, &excluded);
			}
		}

		params
	}

	fn process_text_filter(&self, params: &mut Vec<(&'static str, String)>, id: &str, value: &str) {
		let trimmed = value.trim();
		if trimmed.is_empty() {
			return;
		}

		let parsed_value = match trimmed.parse::<i32>() {
			Ok(val) => val,
			Err(_) => return,
		};

		let (param_name, is_valid) = match id {
			"chap_count_min" => ("chap_count_min", parsed_value >= 0),
			"chap_count_max" => ("chap_count_max", true),
			"year_min" => ("year_min", parsed_value >= 1930),
			"year_max" => ("year_max", true),
			"rating_min" => ("rating_min", parsed_value >= 0),
			"rating_max" => ("rating_max", parsed_value <= 10),
			"rate_min" => ("rate_min", parsed_value >= 0),
			"rate_max" => ("rate_max", true),
			_ => return,
		};

		if is_valid {
			params.push((param_name, trimmed.to_string()));
		}
	}

	fn process_sort_filter(
		&self,
		params: &mut Vec<(&'static str, String)>,
		id: &str,
		index: i32,
		ascending: bool,
	) {
		if id != "sort" || (index == 0 && !ascending) {
			return;
		}

		let sort_value = match index {
			0 => None,                    // По популярности - default, don't include
			1 => Some("rate_avg"),        // По рейтингу
			2 => Some("views"),           // По просмотрам
			3 => Some("chap_count"),      // Количеству глав
			4 => Some("releaseDate"),     // Дате релиза
			5 => Some("last_chapter_at"), // Дате обновления
			6 => Some("created_at"),      // Дате добавления
			7 => Some("name"),            // По названию (A-Z)
			8 => Some("rus_name"),        // По названию (А-Я)
			_ => None,
		};

		if let Some(sort_value) = sort_value {
			params.push(("sort_by", sort_value.to_string()));
		}

		if ascending {
			params.push(("sort_type", "asc".to_string()));
		}
	}

	fn process_check_filter(&self, params: &mut Vec<(&'static str, String)>, id: &str, value: i32) {
		if value != 0 {
			return;
		}

		let param_name = match id {
			"genres_soft_search" => "genres_soft_search",
			"tags_soft_search" => "tags_soft_search",
			_ => return,
		};

		params.push((param_name, "1".to_string()));
	}

	fn process_multiselect_filter(
		&self,
		params: &mut Vec<(&'static str, String)>,
		id: &str,
		included: &[String],
		excluded: &[String],
	) {
		let (include_param, exclude_param) = match id {
			"age_rating" => ("caution[]", None),
			"type" => ("types[]", None),
			"format" => ("format[]", Some("format_exclude[]")),
			"title_status" => ("status[]", None),
			"translation_status" => ("scanlate_status[]", None),
			"genres" => ("genres[]", Some("genres_exclude[]")),
			"tags" => ("tags[]", Some("tags_exclude[]")),
			_ => return,
		};

		for value in included {
			params.push((include_param, value.clone()));
		}

		if let Some(exclude_param) = exclude_param {
			for value in excluded {
				params.push((exclude_param, value.clone()));
			}
		}
	}
}

impl Default for FilterProcessor {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod test;
