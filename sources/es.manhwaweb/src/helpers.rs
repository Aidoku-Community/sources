use aidoku::{
	ContentRating, Listing, MangaStatus, Result, Viewer,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::models::CreatorDto;
use crate::{API_URL, BASE_URL};

/// Same `Accept` header as the Tachiyomi extension. The image host rejects
/// requests without browser-like headers.
pub const ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/png,image/svg+xml,*/*;q=0.8";

pub fn get_json<T: DeserializeOwned>(path: &str) -> Result<T> {
	Request::get(format!("{API_URL}{path}"))?
		.header("Accept", ACCEPT)
		.header("Referer", &format!("{BASE_URL}/"))
		.json_owned()
}

pub fn manga_url(slug: &str) -> String {
	format!("{BASE_URL}/manhwa/{slug}")
}

/// The last non-empty path segment of a URL or path.
pub fn last_segment(url: &str) -> &str {
	url.trim_end_matches('/').rsplit('/').next().unwrap_or(url)
}

pub fn creator_names(creators: &[CreatorDto]) -> Option<Vec<String>> {
	let names: Vec<String> = creators
		.iter()
		.map(|creator| creator.name.trim())
		.filter(|name| !name.is_empty())
		.map(String::from)
		.collect();
	Some(names).filter(|names| !names.is_empty())
}

pub fn parse_status(status: Option<&str>) -> MangaStatus {
	match status {
		Some("publicandose") => MangaStatus::Ongoing,
		Some("finalizado") => MangaStatus::Completed,
		Some("pausado") => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

pub fn parse_viewer(kind: Option<&str>) -> Viewer {
	match kind {
		Some("manhwa" | "manhua") => Viewer::Webtoon,
		Some("manga") => Viewer::RightToLeft,
		_ => Viewer::Unknown,
	}
}

pub fn parse_content_rating(erotic: Option<&str>, is_ecchi: bool) -> ContentRating {
	match erotic {
		Some("si") => ContentRating::NSFW,
		_ if is_ecchi => ContentRating::Suggestive,
		Some("no") => ContentRating::Safe,
		_ => ContentRating::Unknown,
	}
}

/// `5.0` -> `5`, `5.5` -> `5.5`, like the site's chapter links.
pub fn format_number(number: f32) -> String {
	if number == (number as i64) as f32 {
		format!("{}", number as i64)
	} else {
		format!("{number}")
	}
}

pub fn listing(id: &str, name: &str) -> Listing {
	Listing {
		id: id.into(),
		name: name.into(),
		..Default::default()
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn gets_last_segment() {
		assert_eq!(last_segment("/manga/comic-abc_123"), "comic-abc_123");
		assert_eq!(
			last_segment("https://manhwaweb.com/leer/abc_123-5_01/"),
			"abc_123-5_01"
		);
		assert_eq!(last_segment("abc"), "abc");
	}

	#[aidoku_test]
	fn formats_chapter_numbers() {
		assert_eq!(format_number(5.0), "5");
		assert_eq!(format_number(5.5), "5.5");
	}
}
