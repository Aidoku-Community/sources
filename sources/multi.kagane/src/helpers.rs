use aidoku::{
	ContentRating, MangaStatus, Viewer,
	alloc::{string::String, vec, vec::Vec},
	imports::defaults::defaults_get,
};

/// The content languages to request from the API, from the "Languages"
/// setting. Falls back to English when the setting is unset.
fn languages() -> Vec<String> {
	defaults_get::<Vec<String>>("languages").unwrap_or_else(|| vec![String::from("en")])
}

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
	let mut body = serde_json::Map::new();

	if let Some(q) = query.filter(|q| !q.is_empty()) {
		body.insert(String::from("title"), serde_json::json!(q));
	}

	body.insert(String::from("content_lang"), serde_json::json!(languages()));
	body.insert(String::from("source_type"), serde_json::json!(source_types()));
	body.insert(
		String::from("content_rating"),
		serde_json::json!(content_ratings()),
	);

	if !statuses.is_empty() {
		body.insert(String::from("upload_status"), serde_json::json!(statuses));
	}

	if !formats.is_empty() {
		body.insert(String::from("format"), serde_json::json!(formats));
	}

	serde_json::to_string(&serde_json::Value::Object(body)).unwrap_or_default()
}
