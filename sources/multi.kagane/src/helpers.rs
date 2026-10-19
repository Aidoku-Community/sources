use aidoku::{
	ContentRating, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{
		format,
		string::{String, ToString},
		vec,
		vec::Vec,
	},
	helpers::uri::encode_uri_component,
	imports::{
		defaults::defaults_get,
		net::{Request, RequestError, Response},
	},
	prelude::*,
};

use crate::models::{
	GroupResults, SearchResponse, SourceList, StaffResults, TagEntry, UploadSource,
};

/// The mirror picked in the app; the first url in source.json until one is.
pub fn base_url() -> String {
	defaults_get::<String>("url").unwrap_or_else(|| String::from("https://kagane.cx"))
}

pub fn api_base() -> String {
	format!("{}/api/v2", base_url())
}

/// Sort field by the position of the option in `res/filters.json`, as the site lists them.
pub const SORT_PARAMS: &[&str] = &[
	"",                // 0: Relevance
	"avg_views_week",  // 1: Most Read This Week
	"popular_now",     // 2: Popular Now
	"avg_views_month", // 3: Most Read This Month
	"avg_views",       // 4: Most Read of All Time
	"updated_at",      // 5: Last Updated
	"rating",          // 6: Rating
	"created_at",      // 7: Date Added
];

/// An empty sort asks for relevance.
pub fn search_url(page: i32, size: i32, sort: &str, ascending: bool) -> String {
	let mut url = format!("{}/search/series?page={}&size={size}", api_base(), page - 1);
	if !sort.is_empty() {
		let order = if ascending { "asc" } else { "desc" };
		url.push_str(&format!("&sort={sort},{order}"));
	}
	url
}

pub fn sources_request() -> Result<Request> {
	api_post(
		&format!("{}/sources/list", api_base()),
		String::from(r#"{"source_types":null}"#),
	)
}

fn hide_disturbing() -> bool {
	defaults_get::<bool>("hideDisturbing").unwrap_or(true)
}

pub fn show_source() -> bool {
	defaults_get::<bool>("showSourceInTitle").unwrap_or_default()
}

pub fn find_source<'a>(sources: &'a [UploadSource], id: Option<&str>) -> Option<&'a UploadSource> {
	sources
		.iter()
		.find(|source| Some(source.source_id.as_str()) == id)
}

/// The title, followed by its source's name when `sources` holds it.
pub fn source_title(title: &str, sources: &[UploadSource], id: Option<&str>) -> String {
	let title = title.trim();
	match find_source(sources, id) {
		Some(source) => format!("{title} [{}]", source.title),
		None => String::from(title),
	}
}

/// How the series' chapters are credited: its source, ticked when it publishes officially.
pub fn scanlator(source: &UploadSource) -> String {
	match source.source_type.as_deref() {
		Some("Official") => format!("{} ✓", source.title),
		_ => source.title.clone(),
	}
}

/// One search page, with the upload sources fetched alongside when titles name them.
pub fn search_page(url: &str, body: String) -> Result<MangaPageResult> {
	let mut requests = vec![api_post(url, body)?];
	if show_source() {
		requests.push(sources_request()?);
	}
	let mut responses = Request::send_all(requests).into_iter();
	let page: SearchResponse = responses
		.next()
		.ok_or_else(|| error!("Kagane sent no response"))??
		.get_json_owned()?;
	let sources = parse_sources(responses.next());
	Ok(MangaPageResult {
		has_next_page: !page.last,
		entries: page
			.content
			.into_iter()
			.map(|item| item.into_manga(&sources))
			.collect(),
	})
}

/// The sources from their response, or none when it failed; titles and credits then go
/// without them.
pub fn parse_sources(
	response: Option<core::result::Result<Response, RequestError>>,
) -> Vec<UploadSource> {
	response
		.and_then(|response| response.ok()?.get_json_owned::<SourceList>().ok())
		.map(|list| list.sources)
		.unwrap_or_default()
}

pub fn api_get(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Accept", "application/json")
		.header("Origin", &base_url())
		.header("Referer", &format!("{}/", base_url())))
}

pub fn api_post(url: &str, body: String) -> Result<Request> {
	Ok(Request::post(url)?
		.header("Accept", "application/json")
		.header("Content-Type", "application/json")
		.header("Origin", &base_url())
		.header("Referer", &format!("{}/", base_url()))
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

/// Series are rated by age band, with a flag on the pornographic ones; the older single
/// rating is the fallback.
pub fn parse_content_rating(
	age: Option<&str>,
	pornographic: Option<bool>,
	legacy: Option<&str>,
) -> ContentRating {
	match (age, pornographic) {
		(_, Some(true)) | (Some("EighteenPlus"), _) => return ContentRating::NSFW,
		(Some("FifteenPlus"), _) => return ContentRating::Suggestive,
		(Some("Safe"), _) => return ContentRating::Safe,
		_ => {}
	}
	let lower = legacy.map(|s| s.to_lowercase());
	match lower.as_deref() {
		Some("safe") => ContentRating::Safe,
		Some("suggestive") => ContentRating::Suggestive,
		Some("erotica") | Some("pornographic") => ContentRating::NSFW,
		_ => ContentRating::Suggestive,
	}
}

/// The site rejects a list of more ids than this.
const MAX_IDS: usize = 50;

#[derive(Default)]
pub struct SearchFilters {
	pub ratings: Vec<String>,
	pub upload_statuses: Vec<String>,
	pub publication_statuses: Vec<String>,
	pub formats: Vec<String>,
	pub genres_included: Vec<String>,
	pub genres_excluded: Vec<String>,
	pub genres_match_any: bool,
	pub tags_included: Vec<String>,
	pub tags_excluded: Vec<String>,
	pub tags_match_any: bool,
	pub sources: Vec<String>,
	pub staff: Option<String>,
	pub group: Option<String>,
	/// Overrides the hide disturbing setting when set.
	pub include_disturbing: Option<bool>,
	/// The API field and its lower and upper bound.
	pub ranges: Vec<(String, Option<f32>, Option<f32>)>,
}

fn checked<'a>(ids: &'a [String], label: &str) -> Result<&'a [String]> {
	if ids.len() > MAX_IDS {
		bail!(
			"Kagane allows at most {MAX_IDS} {label}, and {} are picked",
			ids.len()
		);
	}
	Ok(ids)
}

fn id_filter(
	included: &[String],
	excluded: &[String],
	match_any: bool,
	label: &str,
) -> Result<Option<serde_json::Value>> {
	if included.is_empty() && excluded.is_empty() {
		return Ok(None);
	}
	let mut filter = serde_json::Map::new();
	filter.insert(
		String::from("values"),
		serde_json::json!(checked(included, label)?),
	);
	filter.insert(String::from("match_all"), serde_json::json!(!match_any));
	if !excluded.is_empty() {
		filter.insert(
			String::from("exclude"),
			serde_json::json!(checked(excluded, label)?),
		);
	}
	Ok(Some(serde_json::Value::Object(filter)))
}

pub fn build_search_body(query: Option<&str>, filters: SearchFilters) -> Result<String> {
	let mut body = serde_json::Map::new();

	if let Some(q) = query {
		body.insert(String::from("title"), serde_json::json!(q));
	}

	body.insert(String::from("content_lang"), serde_json::json!(languages()));
	body.insert(
		String::from("source_type"),
		serde_json::json!(source_types()),
	);
	// A rating picked in search overrides the setting for that search.
	let ratings = if filters.ratings.is_empty() {
		content_ratings()
	} else {
		filters.ratings
	};
	// The search takes age bands, which put Erotica and Pornographic together under 18+.
	let mut age_ratings: Vec<&str> = ratings
		.iter()
		.map(|rating| match rating.as_str() {
			"Safe" => "Safe",
			"Suggestive" => "FifteenPlus",
			_ => "EighteenPlus",
		})
		.collect();
	age_ratings.sort_unstable();
	age_ratings.dedup();
	body.insert(String::from("age_rating"), serde_json::json!(age_ratings));
	// The flag applies to every band, so it can only ask for pornographic titles alone.
	let pornographic = ratings.iter().any(|rating| rating == "Pornographic");
	if !pornographic || ratings.len() == 1 {
		body.insert(
			String::from("is_pornographic"),
			serde_json::json!(pornographic),
		);
	}
	if filters
		.include_disturbing
		.map_or_else(hide_disturbing, |include| !include)
	{
		body.insert(String::from("is_disturbing"), serde_json::json!(false));
	}

	for (field, values) in [
		("upload_status", filters.upload_statuses),
		("publication_status", filters.publication_statuses),
		("format", filters.formats),
	] {
		if !values.is_empty() {
			body.insert(String::from(field), serde_json::json!(values));
		}
	}
	if !filters.sources.is_empty() {
		body.insert(
			String::from("source_id"),
			serde_json::json!(checked(&filters.sources, "sources")?),
		);
	}
	if let Some(genres) = id_filter(
		&filters.genres_included,
		&filters.genres_excluded,
		filters.genres_match_any,
		"genres",
	)? {
		body.insert(String::from("genres"), genres);
	}
	if let Some(tags) = id_filter(
		&filters.tags_included,
		&filters.tags_excluded,
		filters.tags_match_any,
		"tags",
	)? {
		body.insert(String::from("tags"), tags);
	}
	for (field, id) in [("staff", filters.staff), ("groups", filters.group)] {
		if let Some(id) = id {
			body.insert(String::from(field), serde_json::json!({ "values": [id] }));
		}
	}
	for (field, min, max) in filters.ranges {
		let mut range = serde_json::Map::new();
		for (bound, value) in [("min", min), ("max", max)] {
			if let Some(value) = value {
				range.insert(String::from(bound), serde_json::json!(value as i64));
			}
		}
		if !range.is_empty() {
			body.insert(field, serde_json::Value::Object(range));
		}
	}

	Ok(serde_json::to_string(&serde_json::Value::Object(body)).unwrap_or_default())
}

/// The one candidate named `name`, preferring an exact match.
fn single_match(candidates: Vec<(String, String)>, name: &str) -> Result<String> {
	let exact: Vec<&(String, String)> = candidates
		.iter()
		.filter(|(_, candidate)| candidate.eq_ignore_ascii_case(name))
		.collect();
	match (exact.as_slice(), candidates.as_slice()) {
		([(id, _)], _) | ([], [(id, _)]) => Ok(id.clone()),
		([], []) => bail!("Kagane has no matches for {name}"),
		_ => bail!("Kagane has several matches for {name}; enter the full name"),
	}
}

pub fn staff_id(name: &str) -> Result<String> {
	let results: StaffResults = api_post(
		&format!("{}/search/staff?size=8", api_base()),
		serde_json::json!({ "name": name }).to_string(),
	)?
	.json_owned()?;
	single_match(
		results
			.content
			.into_iter()
			.map(|staff| (staff.staff_id, staff.name))
			.collect(),
		name,
	)
}

pub fn group_id(name: &str) -> Result<String> {
	let results: GroupResults = api_get(&format!(
		"{}/groups/search?q={}&limit=8",
		api_base(),
		encode_uri_component(name)
	))?
	.json_owned()?;
	single_match(
		results
			.items
			.into_iter()
			.map(|group| (group.group_id, group.title))
			.collect(),
		name,
	)
}

/// Comma-separated tag names, a leading `-` excluding one, as included and excluded ids.
pub fn tag_ids(input: &str) -> Result<(Vec<String>, Vec<String>)> {
	let tags: Vec<TagEntry> = api_get(&format!("{}/tags/list", api_base()))?.json_owned()?;
	let mut included = Vec::new();
	let mut excluded = Vec::new();
	for name in input
		.split(',')
		.map(str::trim)
		.filter(|name| !name.is_empty())
	{
		let (list, name) = match name.strip_prefix('-') {
			Some(name) => (&mut excluded, name.trim()),
			None => (&mut included, name),
		};
		let before = list.len();
		list.extend(
			tags.iter()
				.filter(|tag| tag.tag_name.eq_ignore_ascii_case(name))
				.map(|tag| tag.id.clone()),
		);
		if list.len() == before {
			bail!("Kagane has no tag named {name}");
		}
	}
	Ok((included, excluded))
}
