use aidoku::imports::std::{current_date, parse_date_with_options};
use aidoku::{
	FilterValue,
	alloc::{String, Vec},
	helpers::uri::QueryParameters,
	imports::html::{Element, Html},
	prelude::format,
};
use chrono::{NaiveDate, NaiveDateTime};

use crate::Params;

pub fn extract_f32_from_string(title: &str, text: &str) -> Vec<f32> {
	text.replace(&title, "")
		.chars()
		.filter(|a| (*a >= '0' && *a <= '9') || *a == ' ' || *a == '.' || *a == '+')
		.collect::<String>()
		.split(' ')
		.collect::<Vec<&str>>()
		.into_iter()
		.map(|a: &str| a.parse::<f32>().unwrap_or(-1.0))
		.filter(|a| *a >= 0.0)
		.collect::<Vec<f32>>()
}

pub fn get_tag_id(genre: i64) -> String {
	String::from(match genre {
		1 => "marvel",
		2 => "dc-comics",
		3 => "action",
		4 => "adventure",
		5 => "anthology",
		6 => "anthropomorphic",
		7 => "biography",
		8 => "children",
		9 => "comedy",
		10 => "crime",
		11 => "cyborgs",
		12 => "dark-horse",
		13 => "demons",
		14 => "drama",
		15 => "fantasy",
		16 => "family",
		17 => "fighting",
		18 => "gore",
		19 => "graphic-novels",
		20 => "historical",
		21 => "horror",
		22 => "leading-ladies",
		23 => "literature",
		24 => "magic",
		25 => "manga",
		26 => "martial-arts",
		27 => "mature",
		28 => "mecha",
		29 => "military",
		30 => "movie-cinematic-link",
		31 => "mystery",
		32 => "mythology",
		33 => "psychological",
		34 => "personal",
		35 => "political",
		36 => "post-apocalyptic",
		37 => "pulp",
		38 => "robots",
		39 => "romance",
		40 => "sci-fi",
		41 => "slice-of-life",
		42 => "science-fiction",
		43 => "sport",
		44 => "spy",
		45 => "superhero",
		46 => "supernatural",
		47 => "suspense",
		48 => "thriller",
		49 => "vampires",
		50 => "vertigo",
		51 => "video-games",
		52 => "war",
		53 => "western",
		54 => "zombies",
		_ => "",
	})
}

pub fn text_with_newlines(node: Element) -> String {
	let html = node.html().unwrap_or_default();
	if html.trim().is_empty() {
		return String::new();
	}

	Html::parse(format!("<div>{}</div>", html.replace("<br>", "{{ .LINEBREAK }}")).as_bytes())
		.ok()
		.and_then(|parsed_node| {
			parsed_node
				.select_first("div")
				.and_then(|v| v.text())
				.map(|t| t.replace("{{ .LINEBREAK }}", "\n"))
		})
		.unwrap_or_default()
}

pub fn get_search_url(
	params: &Params,
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> aidoku::Result<String> {
	let mut qs = QueryParameters::new();
	qs.push("q", Some(&query.unwrap_or_default()));
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

	Ok(format!(
		"{}/{}{}{qs}",
		params.base_url,
		(params.search_page)(page),
		if qs.is_empty() { "" } else { "?" }
	))
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

pub fn parse_chapter_date(params: &Params, date: &str) -> i64 {
	let date = date.trim();

	let result = parse_date_with_options(
		date,
		params.datetime_format,
		params.datetime_locale,
		params.datetime_timezone,
	);
	if let Some(result) = result {
		return result;
	}

	let now = current_date();

	if date.contains("today") {
		return now;
	}

	if date.contains("yesterday") || date.contains("qua") || date.contains("يوم واحد") {
		return now - 60 * 60 * 24;
	}

	if date.contains("يوم وايومين") {
		return now - 2 * 60 * 60 * 24; // day before yesterday
	}

	// fall back to parsing relative date
	// returns current date if not a relative date / it fails to parse
	parse_relative_date(date, now, params.time_formats.as_deref())
}

// parses a relative date string (e.g. "21 horas ago")
pub fn parse_relative_date(
	date: &str,
	current_date: i64,
	absolute_formats: Option<&[&str]>,
) -> i64 {
	let absolute_formats: &[&str] =
		absolute_formats.unwrap_or(&["%d/%m/%Y", "%m-%d-%Y", "%Y-%d-%m"]);

	for fmt in absolute_formats {
		if let Ok(d) = NaiveDate::parse_from_str(date, fmt) {
			let dt = NaiveDateTime::new(d, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
			return dt.and_utc().timestamp();
		}
	}

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
			"day", "hari", "jour", "día", "dia", "gün", "ngày", "日", "日前", "天", "أيام",
		],
	) {
		number * DAY
	} else if any_word_in(
		&date_lc,
		&[
			"hour",
			"heure",
			"hora",
			"jam",
			"saat",
			"giờ",
			"時間",
			"時間前",
			"小时",
			"ساعة",
		],
	) {
		number * HOUR
	} else if any_word_in(
		&date_lc,
		&[
			"minute",
			"min",
			"minuto",
			"menit",
			"dakika",
			"phút",
			"分",
			"分前",
			"นาที",
			"دقائق",
		],
	) {
		number * MINUTE
	} else if any_word_in(
		&date_lc,
		&["second", "segundo", "detik", "giây", "秒", "秒前", "วินาที"],
	) {
		number * SECOND
	} else if any_word_in(&date_lc, &["week", "semana", "tuần", "週間"]) {
		number * WEEK
	} else if any_word_in(&date_lc, &["month", "mes", "tháng", "ヶ月", "か月"]) {
		number * MONTH
	} else if any_word_in(&date_lc, &["year", "año", "năm", "年"]) {
		number * YEAR
	} else {
		0
	};

	current_date - offset
}
