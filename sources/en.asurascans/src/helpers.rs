use crate::BASE_URL;
use aidoku::{alloc::string::String, prelude::*};

/// Returns the ID of a manga from a URL.
pub fn get_manga_key(url: &str) -> Option<String> {
	// Asura Scans appends a random string at the end of each series slug
	// The random string is not necessary, along with the trailing '-'

	// remove query parameters
	let path = url.split('?').next().unwrap_or("");

	// find the segment after "series"
	let manga_segment = path
		.split('/')
		.skip_while(|segment| *segment != "comics")
		.nth(1)?;

	// find the last '-' and keep it in the id
	let pos = manga_segment.rfind('-')?;
	Some(manga_segment[..pos].into())
}

/// Returns the ID of a chapter from a URL.
pub fn get_chapter_key(url: &str) -> Option<String> {
	// remove query parameters
	let path = url.split('?').next().unwrap_or("");

	// find the segment after "chapter"
	let chapter_segment = path
		.split('/')
		.skip_while(|segment| *segment != "chapter")
		.nth(1)?;

	// extract only the numeric (and '.') prefix
	let end_pos = chapter_segment
		.find(|c: char| !c.is_numeric() && c != '.')
		.unwrap_or(chapter_segment.len());

	Some(chapter_segment[..end_pos].into())
}

/// Returns full URL of a manga from a manga ID.
pub fn get_manga_url(manga_id: &str) -> String {
	format!("{BASE_URL}/comics/{manga_id}")
}

/// Returns full URL of a chapter from a chapter ID and manga ID.
pub fn get_chapter_url(chapter_id: &str, manga_id: &str) -> String {
	format!("{BASE_URL}/comics/{manga_id}/chapter/{chapter_id}")
}

/// Parses a relative date string (e.g. "21 hours ago").
pub fn parse_relative_date(date: &str, current_date: i64) -> i64 {
	// extract the first number found in the string
	let number = date
		.split_whitespace()
		.find_map(|word| word.parse::<i64>().ok())
		.unwrap_or(0);

	let date_lc = date.to_lowercase();

	const SECOND: i64 = 1;
	const MINUTE: i64 = 60 * SECOND;
	const HOUR: i64 = 60 * MINUTE;
	const DAY: i64 = 24 * HOUR;
	const WEEK: i64 = 7 * DAY;
	const MONTH: i64 = 30 * DAY;
	const YEAR: i64 = 365 * DAY;

	let offset = if date_lc.contains("day") {
		number * DAY
	} else if date_lc.contains("hour") {
		number * HOUR
	} else if date_lc.contains("min") {
		number * MINUTE
	} else if date_lc.contains("sec") {
		number * SECOND
	} else if date_lc.contains("week") {
		number * WEEK
	} else if date_lc.contains("month") {
		number * MONTH
	} else if date_lc.contains("year") {
		number * YEAR
	} else {
		0
	};

	current_date - offset
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_manga_keys() {
		assert_eq!(
			get_manga_key("https://asurascans.com/comics/swordmasters-youngest-son-cb22671f")
				.as_deref(),
			Some("swordmasters-youngest-son")
		);
		assert_eq!(
			get_manga_key(
				"https://asurascans.com/comics/swordmasters-youngest-son-cb22671f?blahblah"
			)
			.as_deref(),
			Some("swordmasters-youngest-son")
		);
		assert_eq!(
			get_manga_key(
				"https://asurascans.com/comics/swordmasters-youngest-son-cb22671f/chapter/1"
			)
			.as_deref(),
			Some("swordmasters-youngest-son")
		);
	}

	#[aidoku_test]
	fn test_chapter_keys() {
		assert_eq!(
			get_chapter_key("https://asurascans.com/comics/swordmasters-youngest-son-cb22671f"),
			None
		);
		assert_eq!(
			get_chapter_key(
				"https://asurascans.com/comics/swordmasters-youngest-son-cb22671f?blahblah"
			),
			None
		);
		assert_eq!(
			get_chapter_key(
				"https://asurascans.com/comics/swordmasters-youngest-son-cb22671f/chapter/1"
			)
			.as_deref(),
			Some("1")
		);
		assert_eq!(
			get_chapter_key("https://asurascans.com/comics/swordmasters-youngest-son/chapter/1")
				.as_deref(),
			Some("1")
		);
	}
}
