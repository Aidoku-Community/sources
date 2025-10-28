use crate::{LoadMoreStrategy, Params};
use aidoku::{
	FilterValue, Result,
	alloc::{String, string::ToString, vec::Vec},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element},
		net::Request,
		std::{current_date, parse_date_with_options},
	},
	prelude::*,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadMoreDetection {
	Pending,
	True,
	False,
}

// wasm is run single-threaded, so this should be safe
static mut LOAD_MORE_DETECTION: LoadMoreDetection = LoadMoreDetection::Pending;

pub fn should_use_load_more(params: &Params) -> bool {
	match params.use_load_more_request {
		LoadMoreStrategy::Always => true,
		LoadMoreStrategy::Never => false,
		LoadMoreStrategy::AutoDetect => unsafe { LOAD_MORE_DETECTION == LoadMoreDetection::True },
	}
}

pub fn detect_load_more(params: &Params, document: &Document) {
	if params.use_load_more_request == LoadMoreStrategy::AutoDetect
		&& unsafe { LOAD_MORE_DETECTION == LoadMoreDetection::Pending }
	{
		let result = document.select_first("nav.navigation-ajax").is_some();
		unsafe {
			LOAD_MORE_DETECTION = if result {
				LoadMoreDetection::True
			} else {
				LoadMoreDetection::False
			}
		};
	}
}

pub trait ElementImageAttr {
	fn img_attr(&self, use_style: bool) -> Option<String>;
}

impl ElementImageAttr for Element {
	fn img_attr(&self, use_style: bool) -> Option<String> {
		let style_url = || {
			self.attr("style")
				.and_then(|s| extract_between(&s, "url(", ")").map(|s| s.into()))
		};
		let fallback = || {
			self.attr("data-src")
				.or_else(|| self.attr("data-lazy-src"))
				.or_else(|| {
					self.attr("srcset").and_then(|srcset| {
						srcset
							.split(" ")
							.filter(|s| s.starts_with("http"))
							.max() // find longest url (highest quality)
							.map(|s| s.into())
					})
				})
				.or_else(|| self.attr("data-cfsrc"))
				.or_else(|| self.attr("src"))
				.map(|s| s.trim().into())
		};
		if use_style {
			style_url().or_else(fallback)
		} else {
			fallback()
		}
	}
}

pub fn find_first_f32(s: &str) -> Option<f32> {
	let mut num = String::new();
	let mut found_digit = false;
	let mut dot_found = false;

	for c in s.chars() {
		if c.is_ascii_digit() {
			num.push(c);
			found_digit = true;
		} else if c == '.' && found_digit && !dot_found {
			num.push(c);
			dot_found = true;
		} else if found_digit {
			break;
		}
	}

	if found_digit {
		num.parse::<f32>().ok()
	} else {
		None
	}
}

pub fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
	s.find(start).and_then(|start_idx| {
		let after_start = &s[start_idx + start.len()..];
		after_start.find(end).map(|end_idx| &after_start[..end_idx])
	})
}

pub fn get_search_request(
	params: &Params,
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> Result<Request> {
	let mut qs = QueryParameters::new();
	qs.push("s", Some(&query.unwrap_or_default()));
	qs.push("post_type", Some("wp-manga"));
	for filter in filters {
		match filter {
			FilterValue::Text { id, value } => qs.push(&id, Some(&value)),
			FilterValue::Sort { id, index, .. } => {
				let value = match index {
					0 => "",
					1 => "latest",
					2 => "alphabet",
					3 => "rating",
					4 => "trending",
					5 => "views",
					6 => "new-manga",
					_ => "",
				};
				qs.push(&id, Some(value));
			}
			FilterValue::Select { id, value } => {
				qs.push(&id, Some(&value));
			}
			FilterValue::MultiSelect { id, included, .. } => {
				for tag in included {
					qs.push(&id, Some(&tag));
				}
			}
			_ => {}
		}
	}
	let url = format!(
		"{}/{}{}{qs}",
		params.base_url,
		(params.search_page)(page),
		if qs.is_empty() { "" } else { "?" }
	);
	Ok(Request::get(url)?)
}

pub fn get_search_load_more_request(
	params: &Params,
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> Result<Request> {
	let mut qs = QueryParameters::new();
	qs.push("action", Some("madara_load_more"));
	qs.push("page", Some(&(page - 1).to_string()));
	qs.push("template", Some("madara-core/content/content-search"));
	qs.push("vars[paged]", Some("1"));
	qs.push("vars[template]", Some("archive"));
	qs.push("vars[sidebar]", Some("right"));
	qs.push("vars[post_type]", Some("wp-manga"));
	qs.push("vars[post_status]", Some("publish"));
	qs.push("vars[manga_archives_item_layout]", Some("big_thumbnail"));
	if params.filter_non_manga_items {
		qs.push("vars[meta_query][0][key]", Some("_wp_manga_chapter_type"));
		qs.push("vars[meta_query][0][value]", Some("manga"));
	}
	if let Some(query) = query {
		qs.push("vars[s]", Some(&query));
	}

	let mut meta_query_idx = if params.filter_non_manga_items { 1 } else { 0 };
	let mut tax_query_idx = 0;

	for filter in filters {
		match filter {
			FilterValue::Text { id, value } => {
				qs.push(
					&format!("vars[tax_query][{tax_query_idx}][taxonomy]"),
					Some(&format!("wp-manga-{id}")),
				);
				qs.push(
					&format!("vars[tax_query][{tax_query_idx}][field]"),
					Some("name"),
				);
				qs.push(
					&format!("vars[tax_query][{tax_query_idx}][terms]"),
					Some(&value),
				);
				tax_query_idx += 1;
			}
			FilterValue::Sort {
				index, ascending, ..
			} => {
				let asc = if ascending { "ASC" } else { "DESC" };
				match index {
					0 => continue,
					1 => {
						// latest
						qs.push("vars[orderby]", Some("meta_value_num"));
						qs.push("vars[meta_key]", Some("_latest_update"));
						qs.push("vars[order]", Some(asc));
					}
					2 => {
						// alphabet
						qs.push("vars[orderby]", Some("post_title"));
						qs.push("vars[order]", Some(if ascending { "DESC" } else { "ASC" }));
					}
					3 => {
						// rating
						qs.push("vars[orderby][query_average_reviews]", Some(asc));
						qs.push("vars[orderby][query_total_reviews]", Some(asc));
					}
					4 => {
						// trending
						qs.push("vars[orderby]", Some("meta_value_num"));
						qs.push("vars[meta_key]", Some("_wp_manga_week_views_value"));
						qs.push("vars[order]", Some(asc));
					}
					5 => {
						// views
						qs.push("vars[orderby]", Some("meta_value_num"));
						qs.push("vars[meta_key]", Some("_wp_manga_views"));
						qs.push("vars[order]", Some(asc));
					}
					6 => {
						// new
						qs.push("vars[orderby]", Some("date"));
						qs.push("vars[order]", Some(asc));
					}
					_ => continue,
				};
			}
			FilterValue::Select { id, value } => match id.as_str() {
				"adult" => {
					qs.push(
						&format!("vars[meta_query][{meta_query_idx}][key]"),
						Some("manga_adult_content"),
					);
					qs.push(
						&format!("vars[meta_query][{meta_query_idx}][compare]"),
						Some(if value == "0" { "not exists" } else { "exists" }),
					);
					meta_query_idx += 1;
				}
				"op" => {
					if value == "1" {
						qs.push(
							&format!("vars[tax_query][{tax_query_idx}][operation]"),
							Some("AND"),
						);
					}
				}
				_ => continue,
			},
			FilterValue::MultiSelect { id, included, .. } => match id.as_str() {
				"genre[]" => {
					qs.push(
						&format!("vars[tax_query][{tax_query_idx}][taxonomy]"),
						Some("wp-manga-genre"),
					);
					qs.push(
						&format!("vars[tax_query][{tax_query_idx}][field]"),
						Some("slug"),
					);
					for (idx, id) in included.iter().enumerate() {
						qs.push(
							&format!("vars[tax_query][{tax_query_idx}][terms][{idx}]"),
							Some(id),
						);
					}
					tax_query_idx += 1;
				}
				"status[]" => {
					qs.push(
						&format!("vars[meta_query][{meta_query_idx}][key]"),
						Some("_wp_manga_status"),
					);
					for (idx, id) in included.iter().enumerate() {
						qs.push(
							&format!("vars[meta_query][{meta_query_idx}][value][{idx}]"),
							Some(id),
						);
					}
					meta_query_idx += 1;
				}
				_ => continue,
			},
			_ => {}
		}
	}
	let url = format!("{}/wp-admin/admin-ajax.php", params.base_url);
	Ok(Request::post(url)?
		.body(qs.to_string())
		.header("X-Requested-With", "XMLHttpRequest")
		.header("Content-Type", "application/x-www-form-urlencoded"))
}

// parses chapter date string (either relative or with the configured format)
pub fn parse_chapter_date(params: &Params, date: &str) -> i64 {
	let result = parse_date_with_options(
		date,
		&params.datetime_format,
		&params.datetime_locale,
		&params.datetime_timezone,
	);
	if let Some(result) = result {
		return result;
	}

	let now = current_date();

	if date.contains("today") {
		return now;
	}

	if date.contains("yesterday") || date.contains("يوم واحد") {
		return now - 60 * 60 * 24;
	}

	if date.contains("يوم وايومين") {
		return now - 2 * 60 * 60 * 24; // day before yesterday
	}

	// fall back to parsing relative date
	// returns current date if not a relative date / it fails to parse
	parse_relative_date(date, now)
}

// parses a relative date string (e.g. "21 horas ago")
pub fn parse_relative_date(date: &str, current_date: i64) -> i64 {
	// extract the first number found in the string
	let number = date
		.split_whitespace()
		.find_map(|word| word.parse::<i64>().ok())
		.unwrap_or(0);

	let date_lc = date.to_lowercase();

	// check if any word in a set is present in the string
	fn any_word_in(haystack: &str, words: &[&str]) -> bool {
		words.iter().any(|&w| haystack.contains(w))
	}

	const SECOND: i64 = 1;
	const MINUTE: i64 = 60 * SECOND;
	const HOUR: i64 = 60 * MINUTE;
	const DAY: i64 = 24 * HOUR;
	const WEEK: i64 = 7 * DAY;
	const MONTH: i64 = 30 * DAY;
	const YEAR: i64 = 365 * DAY;

	let offset = if any_word_in(
		&date_lc,
		&[
			"hari",
			"gün",
			"jour",
			"día",
			"dia",
			"day",
			"วัน",
			"ngày",
			"giorni",
			"أيام",
			"天",
		],
	) {
		number * DAY
	} else if any_word_in(
		&date_lc,
		&[
			"jam",
			"saat",
			"heure",
			"hora",
			"hour",
			"ชั่วโมง",
			"giờ",
			"ore",
			"ساعة",
			"小时",
		],
	) {
		number * HOUR
	} else if any_word_in(
		&date_lc,
		&["menit", "dakika", "min", "minute", "minuto", "นาที", "دقائق"],
	) {
		number * MINUTE
	} else if any_word_in(&date_lc, &["detik", "segundo", "second", "วินาที"]) {
		number * SECOND
	} else if any_word_in(&date_lc, &["week", "semana"]) {
		number * WEEK
	} else if any_word_in(&date_lc, &["month", "mes"]) {
		number * MONTH
	} else if any_word_in(&date_lc, &["year", "año"]) {
		number * YEAR
	} else {
		0
	};

	current_date - offset
}

// decodes a hex string into a byte array
pub fn decode_hex(s: &str) -> Option<Vec<u8>> {
	if !s.len().is_multiple_of(2) {
		return None;
	}

	let mut bytes = Vec::with_capacity(s.len() / 2);

	let chars: Vec<_> = s.chars().collect();
	for i in (0..s.len()).step_by(2) {
		let hi = chars[i].to_digit(16)? as u8;
		let lo = chars[i + 1].to_digit(16)? as u8;
		bytes.push((hi << 4) | lo);
	}

	Some(bytes)
}

#[cfg(test)]
mod test {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_decode_hex() {
		use aidoku::alloc::vec;
		assert_eq!(
			decode_hex("642c5182b3040fe8"),
			Some(vec![100, 44, 81, 130, 179, 4, 15, 232])
		);
	}
}
