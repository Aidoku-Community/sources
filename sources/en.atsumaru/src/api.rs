use crate::{
	helpers::{ADULT_GENRES, HENTAI, allows, spotlight_genre, take_document},
	models::{ErrorResponse, FeedResponse, SearchResponse},
	settings::{self, CONTENT_RATINGS, CONTENT_TYPES},
};
use aidoku::{
	MangaPageResult, Result,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::net::{Request, Response},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub const BASE_URL: &str = "https://atsu.moe";
pub const API_URL: &str = "https://atsu.moe/api";
pub const CDN_URL: &str = "https://cdn.atsu.moe/static";
const PAGE_SIZE: i32 = 40;
pub const FEED_PAGE_SIZE: i32 = 24;

pub const CARD_FIELDS: &str =
	"id,title,poster,posterMedium,isAdult,mbContentRating,genreIds,medium,status";

// Sort indices must match filters.json.
const SORTS: [&str; 6] = [
	"views",
	"title",
	"trending",
	"dateAdded",
	"releaseDate",
	"mbRating",
];
const FEEDS: [&str; 9] = [
	"hotUpdates",
	"recentlyUpdated",
	"rising",
	"hotArrivals",
	"mostBookmarked",
	"mostTalkedAbout",
	"bingeWorthy",
	"mostPolarizing",
	"hiddenGems",
];
pub const RANKED: [&str; 3] = ["popular", "mostBookmarked", "mostTalkedAbout"];
const RANDOM: usize = SORTS.len() + FEEDS.len();
const POPULARITY: usize = 0;
const TOP_RATED: usize = 5;

pub fn request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Accept", "application/json")
		.header("Referer", &format!("{BASE_URL}/")))
}

pub fn parse<T: DeserializeOwned>(response: Response) -> Result<T> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		match response.get_json_owned::<ErrorResponse>() {
			Ok(body) => bail!("Atsumaru: {}", body.message),
			Err(_) => bail!("Atsumaru returned status {status}"),
		}
	}
	response.get_json_owned()
}

pub fn get<T: DeserializeOwned>(url: &str) -> Result<T> {
	parse(request(url)?.send()?)
}

fn quote(value: &str) -> String {
	format!("`{}`", value.replace('\\', "\\\\").replace('`', "\\`"))
}

fn list<S: AsRef<str>>(values: &[S]) -> String {
	let quoted = values
		.iter()
		.map(|value| quote(value.as_ref()))
		.collect::<Vec<_>>();
	format!("[{}]", quoted.join(","))
}

// Unrated titles need genre-based exclusions.
fn excluded_rating_genres(ratings: &[String]) -> Vec<&'static str> {
	let mut genres = Vec::new();
	if !allows(ratings, "Pornographic") {
		genres.push(HENTAI);
	}
	if !allows(ratings, "Erotica") {
		genres.extend(ADULT_GENRES);
	}
	genres
}

fn split_types(types: &[String]) -> (Vec<&str>, bool) {
	let comics = types
		.iter()
		.map(String::as_str)
		.filter(|kind| *kind != "Novel")
		.collect();
	(comics, allows(types, "Novel"))
}

pub fn lookup_url(ids: &[&str], fields: &str) -> String {
	let mut query = QueryParameters::new();
	query.push("q", Some("*"));
	query.push("query_by", Some("title"));
	query.push("include_fields", Some(fields));
	query.push("filter_by", Some(&format!("id:={}", list(ids))));
	query.push("per_page", Some(&ids.len().max(1).to_string()));
	format!("{BASE_URL}/collections/manga/documents/search?{query}")
}

#[derive(Default)]
pub struct SearchParams {
	pub query: Option<String>,
	pub sort: usize,
	pub ascending: bool,
	pub seed: u32,
	pub genres: Vec<String>,
	pub excluded_genres: Vec<String>,
	pub any_genre: bool,
	pub tags: Vec<String>,
	pub excluded_tags: Vec<String>,
	pub demographics: Vec<String>,
	pub types: Vec<String>,
	pub statuses: Vec<String>,
	pub ratings: Vec<String>,
	pub adult_only: bool,
	pub author: Option<String>,
	pub years: (Option<f32>, Option<f32>),
	pub chapters: (Option<f32>, Option<f32>),
	pub official: bool,
}

impl SearchParams {
	pub fn new(query: Option<String>) -> Self {
		Self {
			query,
			excluded_genres: settings::hidden_genres(),
			excluded_tags: settings::hidden_tags(),
			types: settings::content_types(),
			ratings: settings::content_ratings(),
			adult_only: settings::adult_only(),
			..Default::default()
		}
	}

	// The novel feed is incomplete; use indexed popularity.
	pub fn novels() -> Self {
		let mut search = Self::new(None);
		search.types.retain(|kind| kind == "Novel");
		search
	}

	pub fn feed_url(&self, id: &str, page: i32, timeframe: Option<&str>) -> Result<String> {
		let mut query = QueryParameters::new();
		match id {
			"genreSpotlight" => query.push("genre", Some(spotlight_genre())),
			"popular" | "mostBookmarked" | "mostTalkedAbout" | "hotUpdates" | "recentlyUpdated"
			| "rising" | "hotArrivals" | "recentlyAdded" | "topRated" | "hiddenGems"
			| "bingeWorthy" | "mostPolarizing" => {}
			_ => bail!("Unknown listing"),
		}
		if let Some(timeframe) = timeframe {
			query.push("timeframe", Some(timeframe));
		}
		let (comics, novels) = split_types(&self.types);
		query.push("offset", Some(&((page - 1) * FEED_PAGE_SIZE).to_string()));
		query.push("limit", Some(&FEED_PAGE_SIZE.to_string()));
		// Feeds AND types with mediums; mixed selections need local filtering.
		if comics.is_empty() {
			query.push("mediums", Some("Novel"));
		} else if !novels {
			query.push("mediums", Some("Comic"));
			if comics.len() < CONTENT_TYPES.len() - 1 {
				query.push("types", Some(&comics.join(",")));
			}
		}
		let mut excluded = self
			.excluded_genres
			.iter()
			.chain(&self.excluded_tags)
			.map(String::as_str)
			.collect::<Vec<_>>();
		if self.adult_only {
			query.push("adult", Some("1"));
		} else {
			// Feed exclusions accept both genre and tag ids.
			excluded.extend(excluded_rating_genres(&self.ratings));
			// Omitting contentRatings enables safe mode.
			query.push("contentRatings", Some(&self.ratings.join(",")));
		}
		if !excluded.is_empty() {
			query.push("excludedTags", Some(&excluded.join(",")));
		}
		Ok(format!("{API_URL}/home2/{id}?{query}"))
	}

	pub fn matches_nothing(&self) -> bool {
		self.types.is_empty() || (self.ratings.is_empty() && !self.adult_only)
	}

	fn push_ids(clauses: &mut Vec<String>, field: &str, ids: &[String], any: bool) {
		if any && !ids.is_empty() {
			clauses.push(format!("{field}:={}", list(ids)));
		} else {
			clauses.extend(ids.iter().map(|id| format!("{field}:={}", quote(id))));
		}
	}

	fn type_clause(&self) -> Option<String> {
		let (comics, novels) = split_types(&self.types);
		if novels && comics.len() == CONTENT_TYPES.len() - 1 {
			return None;
		}
		// Unclassified comics can have no medium.
		let mut parts = Vec::new();
		if comics.len() == CONTENT_TYPES.len() - 1 {
			parts.push("medium:!=[`Novel`]".into());
		} else if !comics.is_empty() {
			parts.push(format!("(medium:!=[`Novel`] && type:={})", list(&comics)));
		}
		if novels {
			parts.push("medium:=[`Novel`]".into());
		}
		Some(format!("({})", parts.join(" || ")))
	}

	fn rating_clauses(&self, clauses: &mut Vec<String>) {
		if self.adult_only {
			clauses.push(format!(
				"(isAdult:=true || mbContentRating:=[`Pornographic`] || genreIds:=[{}])",
				quote(HENTAI)
			));
			return;
		}
		// Exclusions retain unrated titles.
		let genres = excluded_rating_genres(&self.ratings);
		if !allows(&self.ratings, "Pornographic") {
			clauses.push("isAdult:=false".into());
		}
		let unwanted = CONTENT_RATINGS
			.into_iter()
			.filter(|rating| !allows(&self.ratings, rating))
			.collect::<Vec<_>>();
		if !unwanted.is_empty() {
			clauses.push(format!("mbContentRating:!={}", list(&unwanted)));
		}
		if !genres.is_empty() {
			clauses.push(format!("genreIds:!={}", list(&genres)));
		}
		// An unrated title without an adult genre counts as safe.
		if !allows(&self.ratings, "Safe") {
			clauses.push(format!(
				"(mbContentRating:=[`Suggestive`,`Erotica`,`Pornographic`] || genreIds:={} || isAdult:=true)",
				list(&[HENTAI, ADULT_GENRES[0], ADULT_GENRES[1]])
			));
		}
	}

	fn filter_by(&self) -> String {
		let mut clauses = Vec::new();
		Self::push_ids(&mut clauses, "genreIds", &self.genres, self.any_genre);
		if !self.excluded_genres.is_empty() {
			clauses.push(format!("genreIds:!={}", list(&self.excluded_genres)));
		}
		Self::push_ids(&mut clauses, "tagIds", &self.tags, false);
		Self::push_ids(&mut clauses, "tagIds", &self.demographics, true);
		if !self.excluded_tags.is_empty() {
			clauses.push(format!("tagIds:!={}", list(&self.excluded_tags)));
		}
		clauses.extend(self.type_clause());
		if !self.statuses.is_empty() {
			clauses.push(format!("status:={}", list(&self.statuses)));
		}
		if let Some(from) = self.years.0 {
			clauses.push(format!("releaseYear:>={}", from as i32));
		}
		if let Some(to) = self.years.1 {
			clauses.push(format!("releaseYear:<={}", to as i32));
		}
		if let Some(from) = self.chapters.0.filter(|from| *from > 0.0) {
			clauses.push(format!("chapterCount:>={}", from as i32));
		}
		if let Some(to) = self.chapters.1.filter(|to| *to > 0.0) {
			clauses.push(format!("chapterCount:<={}", to as i32));
		}
		if self.official {
			clauses.push("officialTranslation:=true".into());
		}
		if let Some(author) = &self.author {
			clauses.push(format!("authors:{}", quote(author)));
		}
		self.rating_clauses(&mut clauses);
		match self.sort {
			POPULARITY => clauses.push("views:>0".into()),
			TOP_RATED => clauses.push("mbRating:>0".into()),
			_ => {}
		}
		clauses.push("hidden:!=true".into());
		clauses.join(" && ")
	}

	pub fn url(&self, page: i32, ids: &[&str]) -> String {
		let mut query = QueryParameters::new();
		query.push("q", Some(self.query.as_deref().unwrap_or("*")));
		query.push(
			"query_by",
			Some("title,englishTitle,otherNames,authors,acronyms"),
		);
		query.push("query_by_weights", Some("4,3,2,2,1"));
		query.push("num_typos", Some("4,3,2,1,0"));
		query.push("prefix", Some("true,true,true,true,false"));
		query.push("include_fields", Some(CARD_FIELDS));
		let mut filter = self.filter_by();
		if !ids.is_empty() {
			filter.push_str(&format!(" && id:={}", list(ids)));
		}
		query.push("filter_by", Some(&filter));
		query.push("page", Some(&page.to_string()));
		query.push("per_page", Some(&PAGE_SIZE.to_string()));
		if self.query.is_some() {
			query.push("infix", Some("off,off,fallback,off,off"));
		}
		let sort = if self.sort == RANDOM {
			let fields = ["shuffleA", "shuffleB"];
			let seed = self.seed.max(1);
			let first = fields[seed as usize % 2];
			let second = fields[(seed as usize + 1) % 2];
			if seed >> 1 & 1 == 1 {
				format!("_eval({first}:<={seed}):desc,{first}:desc,{second}:desc")
			} else {
				format!("_eval({first}:>={seed}):desc,{first}:asc,{second}:asc")
			}
		} else {
			// Feed lookups discard index order.
			let field = SORTS.get(self.sort).copied().unwrap_or(SORTS[POPULARITY]);
			format!("{field}:{}", if self.ascending { "asc" } else { "desc" })
		};
		query.push("sort_by", Some(&sort));
		format!("{BASE_URL}/collections/manga/documents/search?{query}")
	}

	pub fn page(&self, page: i32, label_novels: bool) -> Result<MangaPageResult> {
		if self.matches_nothing() {
			return Ok(MangaPageResult::default());
		}
		let feed = self
			.sort
			.checked_sub(SORTS.len())
			.and_then(|index| FEEDS.get(index));
		if let Some(id) = feed {
			let response: FeedResponse =
				get(&self.feed_url(id, page, RANKED.contains(id).then_some("all"))?)?;
			let has_next_page = response.items.len() as i32 == FEED_PAGE_SIZE;
			if response.items.is_empty() {
				return Ok(MangaPageResult::default());
			}
			// Apply search-only filters in one lookup; retain feed order.
			let ids = response
				.items
				.iter()
				.map(|item| item.id.as_str())
				.collect::<Vec<_>>();
			let matched: SearchResponse = get(&self.url(1, &ids))?;
			let mut documents = matched.hits.into_iter().map(|hit| hit.document).collect();
			let entries = response
				.items
				.into_iter()
				.filter_map(|item| {
					take_document(&mut documents, &item.id)
						.map(|document| document.into_manga(label_novels))
				})
				.collect();
			return Ok(MangaPageResult {
				entries,
				has_next_page,
			});
		}
		let response: SearchResponse = get(&self.url(page, &[]))?;
		Ok(response.into_page(label_novels))
	}
}
