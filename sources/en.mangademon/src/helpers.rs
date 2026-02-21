use crate::BASE_URL;
use aidoku::{alloc::String, prelude::*};

/// Returns the ID of a manga from a URL.
pub fn get_manga_id(url: &str) -> Option<String> {
	// examples:
	// - https://demonicscans.org/title/Overgeared/chapter/1/2024090306
	// - https://demonicscans.org/manga/Overgeared
	// result: "Overgeared"
	if let Some(start) = url.find("/manga/") {
		let id = &url[start + 7..];
		Some(id.into())
	} else if let Some(start) = url.find("/title/") {
		// remove extra path components
		let mut id = &url[start + 8..];
		if let Some(path_idx) = id.find("/") {
			id = &id[..path_idx];
		}
		Some(id.into())
	} else {
		None
	}
}

/// Returns the ID of a chapter from a URL.
pub fn get_chapter_id(url: &str) -> Option<String> {
	// example: https://demonicscans.org/chaptered.php?manga=4&chapter=1
	// result: "/chaptered.php?manga=4&chapter=1"
	url.find("/chaptered.php").map(|i| url[i..].into())
}

/// Returns full URL of a manga from a manga ID.
pub fn get_manga_url(manga_id: &str) -> String {
	format!("{BASE_URL}/manga/{manga_id}")
}

pub fn get_chapter_url(chapter_id: &str) -> String {
	format!("{BASE_URL}{chapter_id}")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_get_manga_id() {
		let url1 = "https://demonicscans.org/title/Becoming-the-Swordmaster-Rank-Young-Lord-of-the-Sichuan-Tang-Family/chapter/111/1";
		assert_eq!(
			get_manga_id(url1).as_deref(),
			Some("Becoming-the-Swordmaster-Rank-Young-Lord-of-the-Sichuan-Tang-Family")
		);

		let url2 = "https://demonicscans.org/manga/Helmut%253A-The-Forsaken-Child";
		assert_eq!(
			get_manga_id(url2).as_deref(),
			Some("Helmut%253A-The-Forsaken-Child")
		);
	}

	#[test]
	fn test_get_chapter_id() {
		let url1 = "https://demonicscans.org/chaptered.php?manga=11616&chapter=122";
		assert_eq!(
			get_chapter_id(url1).as_deref(),
			Some("/chaptered.php?manga=11616&chapter=122")
		);

		let url2 = "/chaptered.php?manga=4&chapter=1";
		assert_eq!(
			get_chapter_id(url2).as_deref(),
			Some("/chaptered.php?manga=4&chapter=1")
		);

		let url3 = "https://demonicscans.org/manga/Helmut%253A-The-Forsaken-Child";
		assert_eq!(get_chapter_id(url3), None);
	}
}
