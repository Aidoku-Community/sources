use aidoku::{
	ContentRating, Viewer,
	alloc::string::{String, ToString},
	imports::std::get_utc_offset,
};
use chrono::{FixedOffset, NaiveDateTime, TimeZone};

pub fn extract_data_chapter_block(script: &str) -> Option<String> {
	// Regex matches: <hex32>:<value>
	let re = regex::Regex::new("[a-z0-9]{32}:([^\\\\\"\\n]+)").ok()?;

	let caps = re.captures(script)?;

	let value = caps.get(1)?.as_str();

	Some(value.to_string())
}

pub fn capitalize(s: &str) -> String {
	let mut chars = s.chars();

	match chars.next() {
		None => String::new(),
		Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
	}
}

pub fn get_viewer(categories: &[String], category: &str) -> (ContentRating, Viewer) {
	let mut nsfw = ContentRating::Unknown;
	let mut viewer = if category == "manga" {
		Viewer::RightToLeft
	} else {
		Viewer::LeftToRight
	};

	for category in categories {
		match category.to_lowercase().as_str() {
			"smut" | "mature" | "18+" | "adult" => nsfw = ContentRating::NSFW,
			"ecchi" | "16+" => {
				if nsfw != ContentRating::NSFW {
					nsfw = ContentRating::Suggestive
				}
			}
			"webtoon" | "manhwa" | "manhua" => viewer = Viewer::Webtoon,
			"manga" => viewer = Viewer::RightToLeft,
			_ => continue,
		}
	}

	(nsfw, viewer)
}

pub fn parse_datetime_to_timestamp(s: &str) -> Option<i64> {
	// Format "YYYY-MM-DD HH:MM:SS"
	let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
	let offset = FixedOffset::east_opt(get_utc_offset() as i32)?;

	let dt = offset.from_local_datetime(&naive).single()?;
	Some(dt.timestamp())
}

pub fn extract_next_object(input: &str, skip: Option<usize>) -> Option<String> {
	let input = input.replace("\\\"", "\"").replace("\\\\\"", "\\\"");
	let bytes = input.as_bytes();

	let mut start = None;
	let mut brace_count = 0;

	let mut skip = skip.unwrap_or_default();
	for (i, &b) in bytes.iter().enumerate() {
		if b == b'{' {
			if i < skip {
				skip -= 1;
				continue;
			}
			if start.is_none() {
				start = Some(i);
			}
			brace_count += 1;
		} else if b == b'}' && brace_count > 0 {
			brace_count -= 1;
			if brace_count == 0 {
				let s = start.unwrap();
				let json_str = &input[s..=i];
				return Some(json_str.to_string());
			}
		}
	}

	None
}
