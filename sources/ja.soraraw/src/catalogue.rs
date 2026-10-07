use crate::{BASE_URL, helpers::request, models::*};
use aidoku::{
	Manga, MangaPageResult, Result, alloc::vec::Vec, imports::std::current_date, prelude::*,
};

const SEARCH_RESULT_LIMIT: usize = 50;
const CATALOGUE_PAGE_LIMIT: i32 = 40; // current actual count is 15
const CATALOGUE_CACHE_SECONDS: i64 = 60 * 60; // 1 hour

pub struct SearchParams<'a> {
	pub query: Option<&'a str>,
	pub author: Option<&'a str>,
	pub content: Option<&'a str>,
	pub status: Option<&'a str>,
	pub included: &'a [i64],
	pub excluded: &'a [i64],
	pub page: i32,
}

pub struct Catalogue {
	pub entries: Vec<CatalogueEntry>,
	pub fetch_time: i64,
}

impl Catalogue {
	pub fn fetch() -> Result<Self> {
		let mut entries = Vec::new();
		for page in 1..=CATALOGUE_PAGE_LIMIT {
			let response = request(format!("{BASE_URL}/mangas_{page}.json"))?.send()?;
			// final page returns 404 to indicate end
			if response.status_code() != 200 {
				if page == 1 {
					bail!(
						"the catalogue is unreachable: page 1 answered {}",
						response.status_code()
					);
				}
				break;
			}
			let catalogue = response.get_json_owned::<CataloguePage>()?;
			entries.extend(catalogue.list);
		}

		Ok(Self {
			entries,
			fetch_time: current_date(),
		})
	}

	pub fn is_fresh(&self, now: i64) -> bool {
		let elapsed = now - self.fetch_time;
		(0..CATALOGUE_CACHE_SECONDS).contains(&elapsed)
	}

	pub fn filter(&self, params: SearchParams<'_>) -> MangaPageResult {
		let offset = (params.page.max(1) as usize - 1).saturating_mul(SEARCH_RESULT_LIMIT);
		let mut matches = self
			.entries
			.iter()
			.filter(|entry| {
				params.query.is_none_or(|query| entry.matches(query))
					&& params
						.author
						.is_none_or(|author| entry.matches_author(author))
					&& params
						.content
						.is_none_or(|content| entry.is_adult.as_deref() == Some(content))
					&& params
						.status
						.is_none_or(|status| entry.kind.as_deref() == Some(status))
					&& params.included.iter().all(|id| entry.genres.contains(id))
					&& params.excluded.iter().all(|id| !entry.genres.contains(id))
			})
			.skip(offset);
		let results = matches
			.by_ref()
			.take(SEARCH_RESULT_LIMIT)
			.cloned()
			.map(Manga::from)
			.collect();
		MangaPageResult {
			entries: results,
			has_next_page: matches.next().is_some(),
		}
	}
}
