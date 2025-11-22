use crate::BASE_URL;
use aidoku::{alloc::string::String, prelude::*};

/// Returns the ID of a manga from a URL.
pub fn get_manga_key(url: &str) -> Option<String> {
	// remove query parameters
	let path = url.split('?').next().unwrap_or("");

	// find the segment after "series"
	let manga_segment = path
		.split('/')
		.skip_while(|segment| *segment != "series")
		.nth(1)?;

	// return the full segment, including the id at the end
	Some(manga_segment.into())
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
	format!("{BASE_URL}/series/{manga_id}")
}

/// Returns full URL of a chapter from a chapter ID and manga ID.
pub fn get_chapter_url(chapter_id: &str, manga_id: &str) -> String {
	format!("{BASE_URL}/series/{manga_id}/chapter/{chapter_id}")
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_manga_keys() {
		assert_eq!(
			get_manga_key("https://asuracomic.net/series/swordmasters-youngest-son-cb22671f")
				.as_deref(),
			Some("swordmasters-youngest-son-cb22671f")
		);
		assert_eq!(
			get_manga_key(
				"https://asuracomic.net/series/swordmasters-youngest-son-cb22671f?blahblah"
			)
			.as_deref(),
			Some("swordmasters-youngest-son-cb22671f")
		);
		assert_eq!(
			get_manga_key(
				"https://asuracomic.net/series/swordmasters-youngest-son-cb22671f/chapter/1"
			)
			.as_deref(),
			Some("swordmasters-youngest-son-cb22671f")
		);
	}

	#[aidoku_test]
	fn test_chapter_keys() {
		assert_eq!(
			get_chapter_key("https://asuracomic.net/series/swordmasters-youngest-son-cb22671f"),
			None
		);
		assert_eq!(
			get_chapter_key(
				"https://asuracomic.net/series/swordmasters-youngest-son-cb22671f?blahblah"
			),
			None
		);
		assert_eq!(
			get_chapter_key(
				"https://asuracomic.net/series/swordmasters-youngest-son-cb22671f/chapter/1"
			)
			.as_deref(),
			Some("1")
		);
	}
}
