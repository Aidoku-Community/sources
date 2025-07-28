use aidoku::{
	Result,
	alloc::{String, Vec, string::ToString},
	imports::{
		html::{Document, Html},
		net::Request,
	},
};
use chrono::{Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};

pub fn fetch_html(url: &str) -> Result<(Document, Option<String>)> {
	let raw = Request::get(url)?.string()?;
	let html = Html::parse(raw.clone())?;
	// Hack to approximate the current date from the cache update date
	let now_str = raw
		.rfind("<!-- WP Fastest Cache file was created")
		.and_then(|i| raw[i..].split("on ").nth(1))
		.and_then(|s| s.split("-->").next())
		.map(|s| s.replace("@ ", "").trim().to_string());

	Ok((html, now_str))
}

pub fn parse_chapter_title(chapter_str: Option<String>) -> Result<(Option<f32>, Option<String>)> {
	if let Some(ref s) = chapter_str {
		// Expect format "Chapter X" or "Chapter X - Some title"
		let prefix = "Chapter ";
		if s.starts_with(prefix) {
			let rest = &s[prefix.len()..];
			let parts: Vec<&str> = rest.splitn(2, " - ").collect();

			let chapter_number = parts
				.get(0)
				.and_then(|num_str| num_str.trim().parse::<f32>().ok());

			let title = if parts.len() > 1 {
				Some(parts[1].trim().to_string())
			} else {
				None
			};

			return Ok((chapter_number, title));
		}
	}

	Ok((None, None))
}

pub fn parse_date_to_timestamp(date_str: &str, now_str: Option<&str>) -> Option<i64> {
	// Try absolute date: "July 17, 2025"
	if let Ok(date) = NaiveDate::parse_from_str(date_str, "%B %d, %Y") {
		return Some(
			Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0)?)
				.timestamp(),
		);
	}

	if now_str.is_none() {
		return None;
	}

	// Try relative date: "5 minutes ago", "2 days ago"
	let lowered = date_str.to_ascii_lowercase();
	let now = NaiveDateTime::parse_from_str(now_str?, "%B %d, %Y %I:%M %p").ok()?;
	let parts: Vec<&str> = lowered.split_whitespace().collect();
	if parts.len() >= 2 {
		if let Ok(value) = parts[0].parse::<i64>() {
			let unit = parts[1];
			let delta = match unit {
				"minute" | "minutes" => Duration::minutes(value),
				"hour" | "hours" => Duration::hours(value),
				"day" | "days" => Duration::days(value),
				_ => return None,
			};
			return Some((now - delta).and_utc().timestamp());
		}
	}

	None
}

#[cfg(test)]
mod test {
	use crate::helpers::{parse_chapter_title, parse_date_to_timestamp};
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn parse_chapter_title_test() {
		let (chapter_number, title) = parse_chapter_title(Some("Chapter 1 - The Beginning".into()))
			.expect("parse_chapter_title failed");
		assert_eq!(chapter_number, Some(1.0));
		assert_eq!(title, Some("The Beginning".into()));

		let (chapter_number, title) =
			parse_chapter_title(Some("Chapter 2".into())).expect("parse_chapter_title failed");
		assert_eq!(chapter_number, Some(2.0));
		assert!(title.is_none());
	}

	#[aidoku_test]
	fn parse_date_to_timestamp_test() {
		let absolute_date_str = parse_date_to_timestamp("July 17, 2025", None);
		assert_eq!(absolute_date_str.unwrap(), 1752710400);

		let relative_date_str =
			parse_date_to_timestamp("5 minutes ago", Some("July 17, 2025 12:00 pm"));
		assert_eq!(relative_date_str.unwrap(), 1752753300);
	}
}
