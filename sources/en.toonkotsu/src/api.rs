use crate::{
	helpers::allows,
	models::{Series, SeriesList},
	settings,
};
use aidoku::{
	Manga, MangaPageResult, Result,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::net::{Request, Response},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub const BASE_URL: &str = "https://toonkotsu.com";
pub const PAGE_SIZE: i32 = 42;

/// Browse sorts in `filters.json` order, each with its direction sent as `order`. The site's
/// upload and update dates, and its view and follow counts, send the same sort.
pub const SORTS: &[&str] = &[
	"relevance",
	"latest",
	"latest",
	"new",
	"score",
	"popular",
	"popular",
	"chapters",
	"year",
	"title",
];

#[derive(Default)]
pub struct BrowseParams {
	pub page: i32,
	pub query: String,
	pub sort: String,
	pub order: String,
	pub discovery: String,
	pub included_genres: Vec<String>,
	pub excluded_genres: Vec<String>,
	pub genre_mode: String,
	pub exclude_mode: String,
	pub demographics: Vec<String>,
	pub statuses: Vec<String>,
	pub types: Vec<String>,
	pub original_languages: Vec<String>,
	pub author: String,
	pub year_min: String,
	pub year_max: String,
	pub chapters_min: String,
	pub chapters_max: String,
}

impl BrowseParams {
	pub fn new(sort: &str, page: i32) -> Self {
		Self {
			page,
			sort: sort.into(),
			order: "desc".into(),
			excluded_genres: settings::hidden_tags(),
			types: settings::types(),
			..Default::default()
		}
	}

	/// The site skips an empty parameter and spells a true one as `1`.
	fn query(&self) -> QueryParameters {
		let mut query = QueryParameters::new();
		let mut set = |name: &str, value: &str| {
			if !value.is_empty() {
				query.push(name, Some(value));
			}
		};
		set("limit", &PAGE_SIZE.to_string());
		set("page", &self.page.to_string());
		set("sort", &self.sort);
		set("order", &self.order);
		set("q", &self.query);
		set("discovery", &self.discovery);
		set("genres", &self.included_genres.join(","));
		set("exclude", &self.excluded_genres.join(","));
		set("genreMode", &self.genre_mode);
		set("excludeMode", &self.exclude_mode);
		set("demographic", &self.demographics.join(","));
		set("status", &self.statuses.join(","));
		set("type", &self.types.join(","));
		set("originalLanguage", &self.original_languages.join(","));
		set("author", &self.author);
		set("yearFrom", &self.year_min);
		set("yearTo", &self.year_max);
		set("chaptersMin", &self.chapters_min);
		set("chaptersMax", &self.chapters_max);
		if sfw() {
			set("sfw", "1");
		}
		query
	}
}

/// Pornographic titles are kept out of the catalogue unless that rating is on.
pub fn sfw() -> bool {
	!settings::content_ratings()
		.iter()
		.any(|rating| rating == "pornographic")
}

fn site_headers(request: Request) -> Request {
	request
		.header("Accept", "application/json")
		.header("Origin", BASE_URL)
		.header("Referer", &format!("{BASE_URL}/"))
}

pub fn json_request(url: String) -> Result<Request> {
	Ok(site_headers(Request::get(url)?))
}

pub fn browse_request(params: &BrowseParams) -> Result<Request> {
	json_request(format!("{BASE_URL}/cdn/v1/browse?{}", params.query()))
}

/// Reader rankings, which browse does not serve.
pub fn ranking_request(metric: &str, params: &BrowseParams) -> Result<Request> {
	let mut query = params.query();
	query.push("metric", Some(metric));
	json_request(format!("{BASE_URL}/api/v1/rankings?{query}"))
}

/// Curated rankings take their parameters as a posted body.
pub fn discovery_request(metric: &str, params: &BrowseParams) -> Result<Request> {
	let mut body = serde_json::json!({
		"limit": PAGE_SIZE,
		"page": params.page,
		"sfw": sfw(),
		"discovery": metric,
		"metric": metric,
	});
	for (name, values) in [
		("type", &params.types),
		("exclude", &params.excluded_genres),
	] {
		if !values.is_empty() {
			body[name] = values.join(",").into();
		}
	}
	let body = serde_json::to_vec(&serde_json::json!({ "params": body }))
		.map_err(|_| error!("Could not build the Toonkotsu ranking request"))?;
	Ok(
		site_headers(Request::post(format!("{BASE_URL}/api/v1/rankings"))?)
			.header("Content-Type", "application/json")
			.body(body),
	)
}

pub fn series_request(key: &str) -> Result<Request> {
	json_request(format!(
		"{BASE_URL}/cdn/v1/series/{}",
		encode_uri_component(key)
	))
}

pub fn parse<T: DeserializeOwned>(response: Response) -> Result<T> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		bail!("Toonkotsu returned status {status}");
	}
	response.get_json_owned::<T>()
}

/// Paging comes from the catalogue, so a page the ratings filter empties still leads on.
pub fn series_page(
	response: Response,
	into_manga: fn(Series) -> Option<Manga>,
) -> Result<MangaPageResult> {
	let list: SeriesList = parse(response)?;
	Ok(MangaPageResult {
		entries: list
			.items
			.unwrap_or_default()
			.into_iter()
			.filter(allows)
			.filter_map(into_manga)
			.collect(),
		has_next_page: list.has_next_page,
	})
}
