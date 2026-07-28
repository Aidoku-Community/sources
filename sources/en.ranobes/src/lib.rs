#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, Listing,
	ListingProvider, Manga, MangaPageResult, NotificationHandler, Page, PageContent, Result,
	Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::std::send_partial_result,
	prelude::*,
};

mod helpers;
mod models;
mod settings;

use helpers::{
	LISTING_COMPLETED, LISTING_LATEST, LISTING_POPULAR, build_listing_url, encode_list,
	encode_value, extract_chapter_text, fetch_chapter_list, fill_manga_details,
	parse_chapter_path, parse_search_results, push_scroller, request_html,
};

pub const BASE_URL: &str = "https://ranobes.top";

// Confirmed by testing each option on the real site and reading back the
// resulting url (rating was confirmed earlier via a direct example url).
const SORT_IDS: &[&str] = &[
	"date",
	"rating",
	"news_read",
	"comm_num",
	"d.chap-num",
	"d.year",
	"editdate",
];

struct Ranobes;

impl Source for Ranobes {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut genres_in: Vec<String> = Vec::new();
		let mut genres_ex: Vec<String> = settings::hidden_genres();
		let mut langs_in: Vec<String> = Vec::new();
		let mut langs_ex: Vec<String> = settings::hidden_languages();
		let mut status_end: Option<String> = None;
		let mut status_trs: Option<String> = None;
		let mut year_from: Option<f32> = None;
		let mut year_to: Option<f32> = None;
		let mut sort: Option<(&str, bool)> = None;

		for filter in filters {
			match filter {
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genre" => {
					genres_in = included
						.into_iter()
						.filter(|genre| {
							// a hidden genre manually re-included in the
							// filter UI overrides the setting for this search
							if let Some(pos) = genres_ex.iter().position(|g| g == genre) {
								genres_ex.swap_remove(pos);
								false
							} else {
								true
							}
						})
						.collect();
					for genre in excluded {
						if !genres_ex.contains(&genre) {
							genres_ex.push(genre);
						}
					}
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "languages" => {
					langs_in = included
						.into_iter()
						.filter(|lang| {
							if let Some(pos) = langs_ex.iter().position(|l| l == lang) {
								langs_ex.swap_remove(pos);
								false
							} else {
								true
							}
						})
						.collect();
					for lang in excluded {
						if !langs_ex.contains(&lang) {
							langs_ex.push(lang);
						}
					}
				}
				FilterValue::Select { id, value } if id == "status-end" && value != "any" => {
					status_end = Some(value);
				}
				FilterValue::Select { id, value } if id == "status-trs" && value != "any" => {
					status_trs = Some(value);
				}
				FilterValue::Range { id, from, to } if id == "year" => {
					year_from = from;
					year_to = to;
				}
				FilterValue::Sort {
					id,
					index,
					ascending,
				} if id == "sort" => {
					sort = SORT_IDS.get(index as usize).map(|s| (*s, ascending));
				}
				_ => {}
			}
		}

		let query = query.filter(|q| !q.is_empty());

		let has_filters = query.is_some()
			|| !genres_in.is_empty()
			|| !genres_ex.is_empty()
			|| !langs_in.is_empty()
			|| !langs_ex.is_empty()
			|| status_end.is_some()
			|| status_trs.is_some()
			|| year_from.is_some()
			|| year_to.is_some()
			|| sort.is_some();

		// Confirmed the site can combine fuzzy title search (`l.title=`)
		// with the rest of the /f/ filter segments in one request, so
		// there's no need to branch between /search/ and /f/ like before —
		// the query is just one more optional segment.
		let html = if has_filters {
			let mut segments = Vec::new();
			if let Some(query) = &query {
				segments.push(format!("l.title={}", encode_value(query)));
			}
			if !genres_in.is_empty() {
				segments.push(format!("n.genre={}", encode_list(&genres_in)));
			}
			if !genres_ex.is_empty() {
				segments.push(format!("v.genre={}", encode_list(&genres_ex)));
			}
			if !langs_in.is_empty() {
				segments.push(format!("b.languages={}", encode_list(&langs_in)));
			}
			if !langs_ex.is_empty() {
				segments.push(format!("v.languages={}", encode_list(&langs_ex)));
			}
			if let Some(status) = &status_end {
				segments.push(format!("status-end={status}"));
			}
			if let Some(status) = &status_trs {
				segments.push(format!("status-trs={status}"));
			}
			if let Some(from) = year_from {
				segments.push(format!("f.year={from}"));
			}
			if let Some(to) = year_to {
				segments.push(format!("t.year={to}"));
			}
			if let Some((sort_id, ascending)) = sort {
				segments.push(format!("sort={sort_id}"));
				segments.push(format!(
					"order={}",
					if ascending { "asc" } else { "desc" }
				));
			}
			if page > 1 {
				segments.push(format!("page/{page}"));
			}
			let url = format!("{BASE_URL}/f/{}/", segments.join("/"));
			request_html(&url)?
		} else {
			let url = format!("{BASE_URL}/search//");
			request_html(&url)?
		};

		let entries = parse_search_results(&html);
		// Matches en.batcave's convention for the same engine: no page count
		// is available from this endpoint, so presence of results is used
		// as a proxy for "more pages might exist".
		let has_next_page = !entries.is_empty();
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{BASE_URL}{}", manga.key);
			let html = request_html(&url)?;
			manga = fill_manga_details(&html, manga)?;
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(fetch_chapter_list(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = request_html(&url)?;
		let text = extract_chapter_text(&html)?;
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl DeepLinkHandler for Ranobes {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split(['?', '#'])
			.next()
			.unwrap_or(&url)
			.strip_prefix(BASE_URL);
		let Some(path) = path else {
			return Ok(None);
		};

		if path.starts_with("/novels/") {
			return Ok(Some(DeepLinkResult::Manga {
				key: path.to_string(),
			}));
		}

		// Chapter urls have a different shape: `/{slug}-{id}/{chapter}.html`,
		// with the novel id at the *end* of the first segment. Confirmed
		// against two different novels that the slug is identical to the
		// one used in the `/novels/{id}-{slug}.html` detail url, just
		// reordered, so the manga key can be reconstructed directly without
		// an extra request.
		if let Some((slug, novel_id)) = parse_chapter_path(path) {
			let manga_key = format!("/novels/{novel_id}-{slug}.html");
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: path.to_string(),
			}));
		}

		Ok(None)
	}
}

impl Home for Ranobes {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut components = Vec::new();
		push_scroller(&mut components, "Latest Updates", LISTING_LATEST)?;
		push_scroller(&mut components, "Popular", LISTING_POPULAR)?;
		push_scroller(&mut components, "Completed", LISTING_COMPLETED)?;
		Ok(HomeLayout { components })
	}
}

impl ListingProvider for Ranobes {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let Some(url) = build_listing_url(&listing.id, page) else {
			bail!("Unknown listing: {}", listing.id);
		};
		let html = request_html(&url)?;
		let entries = parse_search_results(&html);
		let has_next_page = !entries.is_empty();
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

impl NotificationHandler for Ranobes {
	fn handle_notification(&self, notification: String) {
		match notification.as_str() {
			"resetGenreFilter" => settings::reset_hidden_genres(),
			"resetLanguageFilter" => settings::reset_hidden_languages(),
			_ => {}
		}
	}
}

register_source!(
	Ranobes,
	Home,
	ListingProvider,
	DeepLinkHandler,
	NotificationHandler
);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn search_returns_results() {
		let source = Ranobes;
		let result = source
			.get_search_manga_list(Some("Shadow".into()), 1, Vec::new())
			.expect("search failed");
		assert!(!result.entries.is_empty(), "expected at least one result");
		assert!(
			result
				.entries
				.iter()
				.any(|m| m.title.to_lowercase().contains("shadow")),
			"expected a 'Shadow' result"
		);
	}

	#[aidoku_test]
	fn series_detail_has_many_chapters() {
		let source = Ranobes;
		let manga = Manga {
			key: "/novels/1205249-shadow-slave-v741610.html".into(),
			..Default::default()
		};
		let manga = source
			.get_manga_update(manga, true, true)
			.expect("get_manga_update failed");
		assert_eq!(manga.title, "Shadow Slave");
		assert!(manga.description.is_some());
		assert!(manga.authors.is_some());
		let chapters = manga.chapters.expect("no chapters returned");
		assert!(
			chapters.len() > 100,
			"expected >100 chapters, got {}",
			chapters.len()
		);
	}

	#[aidoku_test]
	fn page_list_returns_text_page() {
		let source = Ranobes;
		let manga = Manga {
			key: "/novels/1205249-shadow-slave-v741610.html".into(),
			..Default::default()
		};
		let chapter = Chapter {
			key: "/shadow-slave-v741610-1205249/2053911.html".into(),
			..Default::default()
		};
		let pages = source
			.get_page_list(manga, chapter)
			.expect("get_page_list failed");
		assert_eq!(pages.len(), 1);
		match &pages[0].content {
			PageContent::Text(text) => assert!(!text.is_empty()),
			_ => panic!("expected PageContent::Text"),
		}
	}

	#[aidoku_test]
	fn deep_link_resolves_series() {
		let source = Ranobes;
		let result = source
			.handle_deep_link(
				"https://ranobes.top/novels/1205249-shadow-slave-v741610.html".into(),
			)
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Manga { key } => {
				assert_eq!(key, "/novels/1205249-shadow-slave-v741610.html");
			}
			_ => panic!("expected Manga deep link"),
		}
	}

	#[aidoku_test]
	fn deep_link_resolves_chapter() {
		let source = Ranobes;
		let result = source
			.handle_deep_link(
				"https://ranobes.top/shadow-slave-v741610-1205249/2053911.html".into(),
			)
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Chapter { manga_key, key } => {
				assert_eq!(manga_key, "/novels/1205249-shadow-slave-v741610.html");
				assert_eq!(key, "/shadow-slave-v741610-1205249/2053911.html");
			}
			_ => panic!("expected Chapter deep link"),
		}
	}
}
