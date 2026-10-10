use crate::{BASE_URL, exact_id, fetch_props, models::*, search_artists, search_parodies, tag_id};
use aidoku::{
	MangaPageResult, Result,
	alloc::{String, Vec, collections::BTreeSet, string::ToString},
	helpers::uri::QueryParameters,
	prelude::*,
};

/// At most this many requests are made for one page when results are all duplicates.
const MAX_REQUESTS_PER_PAGE: usize = 4;
/// Pages are filled from the following readings until they have at least this many books.
const MIN_PAGE_SIZE: usize = 20;

/// Filters shared by every part of a search.
pub struct SearchParams {
	pub kind: String,
	pub status: String,
	pub sort: &'static str,
	pub with: Vec<String>,
	pub without: Vec<String>,
	pub match_any: bool,
}

impl SearchParams {
	/// `tag` is the id of a tag, artist or parody the results must have.
	pub fn url(&self, query: &str, tag: Option<&str>, page: i32) -> String {
		let mut with = self.with.join(",");
		if let Some(tag) = tag {
			if !with.is_empty() {
				with.push(',');
			}
			with.push_str(tag);
		}
		let without = self.without.join(",");
		let page = page.to_string();
		// A required tag would become optional when matching any tag.
		let match_any = self.match_any && tag.is_none();
		let mut qs = QueryParameters::new();
		for (name, value) in [
			("q", query),
			("type", self.kind.as_str()),
			("status", self.status.as_str()),
			("sort", self.sort),
			("with", &with),
			("without", &without),
			("match", if match_any { "any" } else { "" }),
			("page", &page),
		] {
			if !value.is_empty() {
				qs.push(name, Some(value));
			}
		}
		format!("{BASE_URL}/search?{qs}")
	}
}

/// One reading of a search: title matches, or the books of a tag, artist or parody.
struct Part {
	query: String,
	tag: Option<String>,
	total: i32,
	first_page: Option<Paginated>,
}

/// A name can mean several things at once: a title, a tag, an artist and a parody can share it.
/// Every reading is searched, and the results are listed one reading after another, the most
/// specific (fewest results) first, without repeating books.
pub struct NameSearch {
	key: String,
	params: SearchParams,
	parts: Vec<Part>,
	part: usize,
	part_page: i32,
	next_page: i32,
	seen: BTreeSet<String>,
}

impl NameSearch {
	/// Identifies a search, so the following pages continue it.
	pub fn key(params: &SearchParams, query: &str, tapped: Option<&str>) -> String {
		format!(
			"{}#{}",
			params.url(query, None, 0),
			tapped.unwrap_or_default()
		)
	}

	pub fn is_continued_by(&self, key: &str, page: i32) -> bool {
		self.key == key && self.next_page == page
	}

	/// `tapped` is a tag tapped on a series page, which is sent by name only.
	pub fn new(
		key: String,
		params: SearchParams,
		query: &str,
		tapped: Option<&str>,
	) -> Result<Self> {
		let mut parts = Vec::new();
		let mut ids: Vec<String> = Vec::new();

		match tapped {
			Some(name) => {
				ids.extend(tag_id(name));
				ids.extend(exact_id(&search_artists(name)?, name).map(|id| id.to_string()));
				ids.extend(exact_id(&search_parodies(name)?, name).map(|id| id.to_string()));
			}
			None => {
				// Title search results list the tags, artists and parodies matching the query.
				let props: ListProps = fetch_props(&params.url(query, None, 1))?;
				for tag in props.matching_tags {
					if tag.name.eq_ignore_ascii_case(query) {
						ids.push(tag.id.to_string());
					}
				}
				parts.push(Part {
					query: query.into(),
					tag: None,
					total: props.books.total,
					first_page: Some(props.books),
				});
			}
		}

		// The query is the name itself unless a tag was tapped alongside a typed query.
		let tag_query = if tapped.is_some() { query } else { "" };
		ids.dedup();
		for id in ids {
			let books = fetch_props::<ListProps>(&params.url(tag_query, Some(&id), 1))?.books;
			parts.push(Part {
				query: tag_query.into(),
				tag: Some(id),
				total: books.total,
				first_page: Some(books),
			});
		}
		parts.sort_by_key(|part| part.total);

		Ok(Self {
			key,
			params,
			parts,
			part: 0,
			part_page: 1,
			next_page: 1,
			seen: BTreeSet::new(),
		})
	}

	pub fn next(&mut self) -> Result<MangaPageResult> {
		let mut entries = Vec::new();
		let mut requests = 0;
		while entries.len() < MIN_PAGE_SIZE && requests < MAX_REQUESTS_PER_PAGE {
			let Some(part) = self.parts.get_mut(self.part) else {
				break;
			};
			let books = match part.first_page.take() {
				Some(books) => books,
				None => {
					requests += 1;
					fetch_props::<ListProps>(&self.params.url(
						&part.query,
						part.tag.as_deref(),
						self.part_page,
					))?
					.books
				}
			};
			if self.part_page >= books.last_page {
				self.part += 1;
				self.part_page = 1;
			} else {
				self.part_page += 1;
			}
			for book in books.data {
				if !self.seen.contains(&book.url) {
					self.seen.insert(book.url.clone());
					entries.extend(book.into_manga());
				}
			}
		}
		self.next_page += 1;
		Ok(MangaPageResult {
			entries,
			has_next_page: self.part < self.parts.len(),
		})
	}
}
