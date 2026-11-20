use crate::{
	BASE_URL,
	helpers::{get_html, parse_manga_list},
	models::SiteSettings,
};
use aidoku::{
	FilterValue, MangaPageResult, Result,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
};

// Sorts in `filters.json` order.
pub const SORTS: [&str; 6] = ["update", "create", "popular", "rating", "chapters", "az"];

#[derive(Default)]
pub struct BrowseParams {
	query: Option<String>,
	sort: &'static str,
	ascending: bool,
	selects: Vec<(&'static str, String)>,
	genres: Vec<String>,
	without: Vec<String>,
	tags: Vec<String>,
	without_tags: Vec<String>,
	pub settings: SiteSettings,
}

impl BrowseParams {
	pub fn new(query: Option<String>) -> Self {
		Self {
			query,
			sort: SORTS[0],
			settings: SiteSettings::current(),
			..Default::default()
		}
	}

	pub fn filter(&mut self, filter: FilterValue) {
		match filter {
			FilterValue::Sort {
				index, ascending, ..
			} => {
				if let Some(sort) = SORTS.get(index as usize) {
					self.sort = sort;
				}
				self.ascending = ascending;
			}
			FilterValue::Select { id, value } => match id.as_str() {
				// A tapped tag can be a genre or a tag; the tag parameter matches both.
				"genre" => {
					self.unhide(&value);
					self.tags.push(value);
				}
				"type" => {
					self.settings.hidden_types.clear();
					self.selects.push(("type", value));
				}
				"content" => {
					self.settings.hidden_ratings.clear();
					self.selects.push(("content", value));
				}
				"status" => self.selects.push(("status", value)),
				"demographic" => self.selects.push(("demographic", value)),
				"updated" => self.selects.push(("updatedWithin", value)),
				_ => {}
			},
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} => {
				for name in &included {
					self.unhide(name);
				}
				if id == "genre" {
					self.genres.extend(included);
					self.without.extend(excluded);
				} else if id.starts_with("tags.") {
					self.tags.extend(included);
					self.without_tags.extend(excluded);
				}
			}
			FilterValue::Check { id, value: 1 } => match id.as_str() {
				"anime" => self.selects.push(("anime", "1".into())),
				"licensed" => self.selects.push(("licensed", "1".into())),
				_ => {}
			},
			FilterValue::Range { id, from, to } => {
				let (low, high) = match id.as_str() {
					"year" => ("yearFrom", "yearTo"),
					"chapters" => ("minChapters", "maxChapters"),
					"rating" => ("minRating", "maxRating"),
					_ => return,
				};
				for (name, bound) in [(low, from), (high, to)] {
					if let Some(bound) = bound {
						self.selects.push((name, (bound as i32).to_string()));
					}
				}
			}
			FilterValue::Text { id, value } => match id.as_str() {
				"author" => self.selects.push(("author", value)),
				"artist" => self.selects.push(("artist", value)),
				"pages" => self.selects.push(("minPages", value)),
				_ => {}
			},
			_ => {}
		}
	}

	// A tag or genre included in a search overrides its exclusion in settings.
	fn unhide(&mut self, name: &str) {
		self.settings
			.hidden_tags
			.retain(|tag| !tag.eq_ignore_ascii_case(name));
	}

	pub fn url(&self, page: i32) -> String {
		let mut query = QueryParameters::new();
		if self.query.is_some() {
			query.push("q", self.query.as_deref());
		}
		query.push("langs", Some(&self.settings.chapter_langs.join(",")));
		query.push_encoded("sort", Some(self.sort));
		query.push_encoded("dir", Some(if self.ascending { "asc" } else { "desc" }));
		for (name, value) in &self.selects {
			query.push(name, Some(value));
		}
		for (name, values) in [
			("genre", &self.genres),
			("without", &self.without),
			("tags", &self.tags),
			("withoutTags", &self.without_tags),
		] {
			if !values.is_empty() {
				query.push(name, Some(&values.join(",")));
			}
		}
		if page > 1 {
			query.push_encoded("page", Some(&page.to_string()));
		}
		format!("{BASE_URL}/browse?{query}")
	}

	pub fn page(&self, page: i32) -> Result<MangaPageResult> {
		if self.settings.shows_nothing() {
			return Ok(MangaPageResult::default());
		}
		let document = get_html(&self.url(page), &self.settings)?;
		let main = document.select_first("main");
		Ok(MangaPageResult {
			entries: main.as_ref().map(parse_manga_list).unwrap_or_default(),
			has_next_page: main
				.and_then(|main| main.select("nav a[href*='page=']"))
				.into_iter()
				.flatten()
				.any(|link| link.text().is_some_and(|text| text.contains("Next"))),
		})
	}
}
