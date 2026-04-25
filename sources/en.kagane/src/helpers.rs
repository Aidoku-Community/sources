use aidoku::{alloc::String, MangaStatus};
use chrono::{DateTime, NaiveDateTime};
use core::fmt::Write;

pub fn parse_date(s: &str) -> f64 {
	DateTime::parse_from_rfc3339(s)
		.map(|dt| dt.timestamp() as f64)
		.or_else(|_| {
			NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
				.map(|dt| dt.and_utc().timestamp() as f64)
		})
		.unwrap_or(0.0)
}

pub fn status_from_str(s: &str) -> MangaStatus {
	match s.to_ascii_uppercase().as_str() {
		"ONGOING" => MangaStatus::Ongoing,
		"COMPLETED" => MangaStatus::Completed,
		"HIATUS" => MangaStatus::Hiatus,
		"ABANDONED" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn build_chapter_name(
	title: &str,
	chapter_no: Option<&str>,
	volume_no: Option<&str>,
	mode: &str,
) -> String {
	let title = title.trim();
	match mode {
		"optional" => {
			if title.is_empty() {
				if let Some(ch) = chapter_no.filter(|s| !s.is_empty()) {
					let mut s = String::new();
					let _ = write!(s, "Ch.{ch}");
					return s;
				}
			}
			String::from(title)
		}
		"always" => match (chapter_no.filter(|s| !s.is_empty()), title.is_empty()) {
			(None, _) => String::from(title),
			(Some(ch), true) => {
				let mut s = String::new();
				let _ = write!(s, "Ch.{ch}");
				s
			}
			(Some(ch), false) => {
				let mut s = String::new();
				let _ = write!(s, "Ch.{ch} {title}");
				s
			}
		},
		"vol_chapter" | _ => {
			let mut num = String::new();
			if let Some(v) = volume_no.filter(|s| !s.is_empty()) {
				let _ = write!(num, "Vol.{v} ");
			}
			if let Some(ch) = chapter_no.filter(|s| !s.is_empty()) {
				let _ = write!(num, "Ch.{ch}");
			}
			let num = num.trim_end();
			if num.is_empty() {
				String::from(title)
			} else if title.is_empty() {
				String::from(num)
			} else {
				let mut s = String::new();
				let _ = write!(s, "{num} {title}");
				s
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_parse_date_valid() {
		let ts = parse_date("2024-01-15T10:30:00");
		assert!(ts > 0.0, "expected positive timestamp");
	}

	#[aidoku_test]
	fn test_parse_date_rfc3339_with_fractional_seconds() {
		let ts = parse_date("2024-01-15T10:30:00.123456Z");
		assert_eq!(ts as i64, 1_705_314_600);
	}

	#[aidoku_test]
	fn test_parse_date_invalid() {
		let ts = parse_date("not-a-date");
		assert_eq!(ts, 0.0);
	}

	#[aidoku_test]
	fn test_status_ongoing() {
		use aidoku::MangaStatus;
		assert_eq!(status_from_str("ONGOING"), MangaStatus::Ongoing);
		assert_eq!(status_from_str("ongoing"), MangaStatus::Ongoing);
	}

	#[aidoku_test]
	fn test_status_unknown() {
		use aidoku::MangaStatus;
		assert_eq!(status_from_str("WHATEVER"), MangaStatus::Unknown);
	}

	#[aidoku_test]
	fn test_chapter_name_optional_no_number() {
		let name = build_chapter_name("The Beginning", Some("1"), None, "optional");
		assert_eq!(name, "The Beginning");
	}

	#[aidoku_test]
	fn test_chapter_name_optional_no_title() {
		let name = build_chapter_name("", Some("5"), None, "optional");
		assert_eq!(name, "Ch.5");
	}

	#[aidoku_test]
	fn test_chapter_name_always() {
		let name = build_chapter_name("Storm", Some("3"), None, "always");
		assert_eq!(name, "Ch.3 Storm");
	}

	#[aidoku_test]
	fn test_chapter_name_vol_chapter() {
		let name = build_chapter_name("Storm", Some("3"), Some("1"), "vol_chapter");
		assert_eq!(name, "Vol.1 Ch.3 Storm");
	}

	#[aidoku_test]
	fn test_chapter_name_vol_chapter_no_vol() {
		let name = build_chapter_name("Storm", Some("3"), None, "vol_chapter");
		assert_eq!(name, "Ch.3 Storm");
	}
}
