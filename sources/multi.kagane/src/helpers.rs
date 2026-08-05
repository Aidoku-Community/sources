use aidoku::{
	ContentRating, MangaStatus, Result, Viewer,
	alloc::{format, string::String, vec, vec::Vec},
	imports::{defaults::defaults_get, net::Request},
};

pub const BASE_URL: &str = "https://kagane.to";
pub const API_BASE: &str = "https://kagane.to/api/v2";

pub fn api_get(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Origin", BASE_URL)
		.header("Referer", &format!("{BASE_URL}/")))
}

pub fn api_post(url: &str, body: String) -> Result<Request> {
	Ok(Request::post(url)?
		.header("Content-Type", "application/json")
		.header("Origin", BASE_URL)
		.header("Referer", &format!("{BASE_URL}/"))
		.body(body))
}

/// The content languages to request from the API. Reads the app's built-in
/// language selection (populated from the `languages` array in source.json)
/// and maps each canonical code to the code kagane's API expects. Falls back
/// to English when nothing is selected.
fn languages() -> Vec<String> {
	defaults_get::<Vec<String>>("languages")
		.filter(|langs| !langs.is_empty())
		.map(|langs| {
			langs
				.into_iter()
				.map(|lang| match lang.as_str() {
					"pt-BR" => String::from("pt-br"),
					_ => lang,
				})
				.collect()
		})
		.unwrap_or_else(|| vec![String::from("en")])
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

pub fn build_search_body(
	query: Option<&str>,
	statuses: &[String],
	formats: &[String],
	genres_included: &[String],
	genres_excluded: &[String],
) -> String {
	let mut body = serde_json::Map::new();

	if let Some(q) = query.filter(|q| !q.is_empty()) {
		body.insert(String::from("title"), serde_json::json!(q));
	}

	body.insert(String::from("content_lang"), serde_json::json!(languages()));
	body.insert(
		String::from("source_type"),
		serde_json::json!(source_types()),
	);
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

	// Genres are sent as an object with included `values`, `exclude`, and a
	// constant `match_all: true` (the website never toggles it).
	if !genres_included.is_empty() || !genres_excluded.is_empty() {
		let mut genres = serde_json::Map::new();
		genres.insert(String::from("values"), serde_json::json!(genres_included));
		genres.insert(String::from("match_all"), serde_json::json!(true));
		if !genres_excluded.is_empty() {
			genres.insert(String::from("exclude"), serde_json::json!(genres_excluded));
		}
		body.insert(String::from("genres"), serde_json::Value::Object(genres));
	}

	serde_json::to_string(&serde_json::Value::Object(body)).unwrap_or_default()
}
