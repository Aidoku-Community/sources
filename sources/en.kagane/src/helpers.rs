use aidoku::{
	ContentRating, MangaStatus, Viewer,
	alloc::{format, string::String, vec, vec::Vec},
	imports::defaults::defaults_get,
};

/// The content ratings to request from the API, from the "Content Rating"
/// setting. Falls back to Safe + Suggestive when the setting is unset.
fn content_ratings() -> Vec<String> {
	defaults_get::<Vec<String>>("contentRating")
		.unwrap_or_else(|| vec![String::from("Safe"), String::from("Suggestive")])
}

/// The source types to request from the API, from the "Source Type" setting.
/// Falls back to all types when the setting is unset.
fn source_types() -> Vec<String> {
	defaults_get::<Vec<String>>("sourceType").unwrap_or_else(|| {
		vec![
			String::from("Official"),
			String::from("Unofficial"),
			String::from("Mixed"),
		]
	})
}

pub fn parse_status(s: &str) -> MangaStatus {
	match s.to_uppercase().as_str() {
		"ONGOING" => MangaStatus::Ongoing,
		"COMPLETED" => MangaStatus::Completed,
		"HIATUS" => MangaStatus::Hiatus,
		"ABANDONED" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn parse_viewer(format: Option<&str>) -> Viewer {
	match format {
		Some("Manga") => Viewer::RightToLeft,
		Some("Comic") => Viewer::LeftToRight,
		_ => Viewer::Webtoon,
	}
}

pub fn parse_content_rating(s: Option<&str>) -> ContentRating {
	let lower = s.map(|s| s.to_lowercase());
	match lower.as_deref() {
		Some("safe") => ContentRating::Safe,
		Some("suggestive") => ContentRating::Suggestive,
		Some("erotica") | Some("pornographic") => ContentRating::NSFW,
		_ => ContentRating::Suggestive,
	}
}

pub fn build_search_body(query: Option<&str>, statuses: &[String], formats: &[String]) -> String {
	let mut parts: Vec<String> = Vec::new();

	if let Some(q) = query
		&& !q.is_empty()
	{
		parts.push(format!("\"title\":\"{}\"", escape_json(q)));
	}

	parts.push(String::from("\"content_lang\":[\"en\"]"));

	let sources = source_types()
		.iter()
		.map(|s| format!("\"{}\"", escape_json(s)))
		.collect::<Vec<_>>()
		.join(",");
	parts.push(format!("\"source_type\":[{sources}]"));

	let ratings = content_ratings()
		.iter()
		.map(|r| format!("\"{}\"", escape_json(r)))
		.collect::<Vec<_>>()
		.join(",");
	parts.push(format!("\"content_rating\":[{ratings}]"));

	if !statuses.is_empty() {
		let joined = statuses
			.iter()
			.map(|s| format!("\"{}\"", escape_json(s)))
			.collect::<Vec<_>>()
			.join(",");
		parts.push(format!("\"upload_status\":[{joined}]"));
	}

	if !formats.is_empty() {
		let joined = formats
			.iter()
			.map(|s| format!("\"{}\"", escape_json(s)))
			.collect::<Vec<_>>()
			.join(",");
		parts.push(format!("\"format\":[{joined}]"));
	}

	format!("{{{}}}", parts.join(","))
}

fn escape_json(s: &str) -> String {
	let mut out = String::new();
	for ch in s.chars() {
		match ch {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\n' => out.push_str("\\n"),
			'\r' => out.push_str("\\r"),
			'\t' => out.push_str("\\t"),
			c => out.push(c),
		}
	}
	out
}
