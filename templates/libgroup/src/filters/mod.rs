use aidoku::{
	FilterValue,
	alloc::{String, Vec, string::ToString},
	imports::std::current_date,
};
use chrono::{DateTime, Datelike, Utc};

#[derive(Clone)]
pub enum FilterId {
	Sort,
	GenresMatchMode,
	TagsMatchMode,
	AgeRating,
	Type,
	Format,
	TitleStatus,
	TranslationStatus,
	Genres,
	Tags,
	ChapCount,
	Year,
	Rating,
	RateCount,
}

impl From<&str> for FilterId {
	fn from(s: &str) -> Self {
		match s {
			"sort" => Self::Sort,
			"genres_match_mode" => Self::GenresMatchMode,
			"tags_match_mode" => Self::TagsMatchMode,
			"age_rating" => Self::AgeRating,
			"type" => Self::Type,
			"format" => Self::Format,
			"title_status" => Self::TitleStatus,
			"translation_status" => Self::TranslationStatus,
			"genres" => Self::Genres,
			"tags" => Self::Tags,
			"chap_count" => Self::ChapCount,
			"year" => Self::Year,
			"rating" => Self::Rating,
			"rate_count" => Self::RateCount,
			_ => Self::Sort, // Default
		}
	}
}

pub struct FilterProcessor;

impl FilterProcessor {
	pub const fn new() -> Self {
		Self
	}

	pub fn process_filters(&self, filters: Vec<FilterValue>) -> Vec<(&'static str, String)> {
		filters
			.into_iter()
			.flat_map(|f| self.process_filter(f))
			.collect()
	}

	fn process_filter(&self, filter: FilterValue) -> Vec<(&'static str, String)> {
		let mut params = Vec::new();
		match filter {
			FilterValue::Sort {
				id,
				index,
				ascending,
			} => {
				let id_enum = FilterId::from(id.as_str());
				if let FilterId::Sort = id_enum {
					match index {
						1 => params.push(("sort_by", "rate_avg".to_string())),
						2 => params.push(("sort_by", "views".to_string())),
						3 => params.push(("sort_by", "chap_count".to_string())),
						4 => params.push(("sort_by", "releaseDate".to_string())),
						5 => params.push(("sort_by", "last_chapter_at".to_string())),
						6 => params.push(("sort_by", "created_at".to_string())),
						7 => params.push(("sort_by", "name".to_string())),
						8 => params.push(("sort_by", "rus_name".to_string())),
						_ => {}
					}
					if ascending && index > 0 {
						params.push(("sort_type", "asc".to_string()));
					}
				}
			}
			FilterValue::Select { id, value } => {
				let id_enum = FilterId::from(id.as_str());
				match id_enum {
					FilterId::GenresMatchMode if value == "any" => {
						params.push(("genres_soft_search", "1".to_string()))
					}
					FilterId::TagsMatchMode if value == "any" => {
						params.push(("tags_soft_search", "1".to_string()))
					}
					_ => {}
				}
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} => {
				let id_enum = FilterId::from(id.as_str());
				match id_enum {
					FilterId::AgeRating => {
						Self::add_multi(&mut params, "caution[]", &included, None)
					}
					FilterId::Type => Self::add_multi(&mut params, "types[]", &included, None),
					FilterId::Format => Self::add_multi(
						&mut params,
						"format[]",
						&included,
						Some(("format_exclude[]", &excluded)),
					),
					FilterId::TitleStatus => {
						Self::add_multi(&mut params, "status[]", &included, None)
					}
					FilterId::TranslationStatus => {
						Self::add_multi(&mut params, "scanlate_status[]", &included, None)
					}
					FilterId::Genres => Self::add_multi(
						&mut params,
						"genres[]",
						&included,
						Some(("genres_exclude[]", &excluded)),
					),
					FilterId::Tags => Self::add_multi(
						&mut params,
						"tags[]",
						&included,
						Some(("tags_exclude[]", &excluded)),
					),
					_ => {}
				}
			}
			FilterValue::Range { id, from, to } => {
				let id_enum = FilterId::from(id.as_str());
				match id_enum {
					FilterId::ChapCount => {
						if let Some(f) = from {
							params.push(("chap_count_min", f.to_string()));
						}
						if let Some(t) = to {
							params.push(("chap_count_max", t.to_string()));
						}
					}
					FilterId::Year => {
						if let Some(f) = from {
							params.push(("year_min", f.to_string()));
						}
						if let Some(t) = to {
							let now = DateTime::<Utc>::from_timestamp(current_date(), 0).unwrap();
							let current_year = now.year() as f32;

							let clamped_year = t.min(current_year);
							params.push(("year_max", clamped_year.to_string()));
						}
					}
					FilterId::Rating => {
						if let Some(f) = from {
							params.push(("rating_min", f.to_string()));
						}
						if let Some(t) = to {
							params.push(("rating_max", t.to_string()));
						}
					}
					FilterId::RateCount => {
						if let Some(f) = from {
							params.push(("rate_min", f.to_string()));
						}
						if let Some(t) = to {
							params.push(("rate_max", t.to_string()));
						}
					}
					_ => {}
				}
			}
			_ => {}
		}
		params
	}

	fn add_multi(
		params: &mut Vec<(&'static str, String)>,
		key: &'static str,
		values: &[String],
		exclude: Option<(&'static str, &[String])>,
	) {
		params.extend(values.iter().map(|v| (key, v.clone())));
		if let Some((ex_key, ex_values)) = exclude {
			params.extend(ex_values.iter().map(|v| (ex_key, v.clone())));
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
