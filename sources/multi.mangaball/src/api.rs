use crate::{
	helpers::{site_languages, title_type},
	models::{Paginated, Title},
	settings,
};
use aidoku::{
	ContentRating, Manga, MangaPageResult, Result,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::net::{Request, Response},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub const BASE_URL: &str = "https://mangaball.com";
pub const API_URL: &str = "https://mangaball.com/api/v1";

// Sorts in `filters.json` order.
pub const SORTS: [&str; 5] = ["lastupdate", "views", "rating", "created_at", "name"];

#[derive(Default)]
pub struct SearchParams {
	pub query: Option<String>,
	pub sort: &'static str,
	pub ascending: bool,
	pub adult_mode: String,
	pub kind: String,
	pub status: String,
	pub demographic: String,
	pub included_tags: Vec<String>,
	pub excluded_tags: Vec<String>,
	pub tag_mode: String,
	pub languages: Vec<(String, String)>,
	pub excluded_scanlators: Vec<String>,
}

impl SearchParams {
	pub fn new(query: Option<String>) -> Self {
		Self {
			query,
			sort: SORTS[0],
			adult_mode: settings::adult_mode().into(),
			kind: settings::content_type(),
			excluded_tags: settings::excluded_tags(),
			tag_mode: "AND".into(),
			languages: site_languages(settings::languages()),
			excluded_scanlators: settings::excluded_scanlators(),
			..Default::default()
		}
	}

	pub fn sorted(sort: &'static str) -> Self {
		Self {
			sort,
			..Self::new(None)
		}
	}

	pub fn url(&self, page: i32) -> String {
		let mut query = QueryParameters::new();
		let mut set = |name: &str, value: &str| {
			if !value.is_empty() {
				query.push(name, Some(value));
			}
		};
		if let Some(keyword) = &self.query {
			set("keyword", keyword.trim());
		}
		set("sort_by", self.sort);
		set("sort_order", if self.ascending { "asc" } else { "desc" });
		set("adult_mode", &self.adult_mode);
		set("tag_mode", &self.tag_mode);
		let languages = self
			.languages
			.iter()
			.map(|(code, _)| code.as_str())
			.collect::<Vec<_>>();
		set("chapterLanguage", &languages.join(","));
		set("type", &self.kind);
		set("status", &self.status);
		set("publicationDemographic", &self.demographic);
		set("included_tags", &self.included_tags.join(","));
		set("excluded_tags", &self.excluded_tags.join(","));
		set("page", &page.to_string());
		set("limit", "24");
		format!("{API_URL}/title/search-advanced?{query}")
	}

	pub fn page(&self, page: i32) -> Result<MangaPageResult> {
		if self.languages.is_empty() {
			return Ok(MangaPageResult::default());
		}
		let response: Paginated<Title> = get(&self.url(page))?;
		Ok(MangaPageResult {
			entries: response
				.data
				.into_iter()
				.filter(|title| self.allows_rating(title))
				.filter_map(|title| Manga::try_from(title).ok())
				.collect(),
			has_next_page: response.pagination.page < response.pagination.total_pages,
		})
	}

	// The API's safe mode can include adult-tagged titles.
	pub fn allows_rating(&self, title: &Title) -> bool {
		self.adult_mode == "all"
			|| (title.content_rating() == ContentRating::NSFW) == (self.adult_mode == "only_18")
	}

	pub fn allows(&self, title: &Title) -> bool {
		self.allows_rating(title)
			&& (self.kind.is_empty()
				|| title_type(&title.original_language) == Some(self.kind.as_str()))
			&& !title
				.tags
				.iter()
				.any(|tag| self.excluded_tags.contains(&tag.id))
			&& title.translated_languages.iter().any(|language| {
				self.languages
					.iter()
					.any(|(code, _)| *code == language.to_lowercase())
			})
	}
}

pub fn site_headers(request: Request) -> Request {
	request
		.header("Accept", "application/json")
		.header("Referer", &format!("{BASE_URL}/"))
}

pub fn request(url: &str) -> Result<Request> {
	Ok(site_headers(Request::get(url)?))
}

pub fn parse<T: DeserializeOwned>(response: Response) -> Result<T> {
	let status = response.status_code();
	if !(200..300).contains(&status) {
		bail!("MangaBall returned status {status}");
	}
	response.get_json_owned()
}

pub fn get<T: DeserializeOwned>(url: &str) -> Result<T> {
	parse(request(url)?.send()?)
}
