#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext, Result,
	Source, UpdateStrategy, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{defaults::defaults_get, net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;

use helpers::*;

const BASE_URL: &str = "https://mangaraw.best";

struct MangaRawBest;

impl Source for MangaRawBest {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		// Both are only known once every filter has been seen.
		let mut sort = SORT_VALUES[0];
		let mut search_type = String::from("name");

		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(value) = SORT_VALUES.get(index as usize) {
						sort = value;
					}
				}
				FilterValue::Select { id, value } => {
					if id == "search_type" {
						search_type = value;
					}
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"status" => {
						if !included.is_empty() {
							qs.push("filter[status]", Some(&included.join(",")));
						}
					}
					"genre" => {
						if !included.is_empty() {
							qs.push("filter[accept_genres]", Some(&included.join(",")));
						}
						if !excluded.is_empty() {
							qs.push("filter[reject_genres]", Some(&excluded.join(",")));
						}
					}
					_ => {}
				},
				_ => {}
			}
		}

		qs.push("sort", Some(sort));
		qs.push("page", Some(&page.to_string()));

		if let Some(query) = query.as_deref().map(str::trim) {
			qs.push(&format!("filter[{search_type}]"), Some(query));
		}

		self.fetch_manga_page(&qs.to_string(), page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{BASE_URL}/raw/{}", manga.key);
		let html = Request::get(&manga_url)?.html()?;

		if needs_details {
			if let Some(title) = html.select_first("main h1").and_then(|el| el.text()) {
				manga.title = title;
			}
			manga.cover = html
				.select_first(".cover-frame img")
				.and_then(|el| el.attr("abs:src"));
			// The description lives in an inner .manga-pilot box whose paragraphs
			// are nested in invalid markup, so read the container's joined text.
			manga.description = html
				.select_first(".manga-pilot .manga-pilot")
				.and_then(|el| el.text())
				.map(|text| text.trim().into())
				.filter(|text: &String| !text.is_empty());
			manga.url = Some(manga_url);

			let tags = parse_tags(&html);
			manga.content_rating = content_rating_from_tags(&tags);
			manga.tags = (!tags.is_empty()).then_some(tags);

			manga.status = parse_status(&html);
			manga.update_strategy = match manga.status {
				MangaStatus::Completed => UpdateStrategy::Never,
				_ => UpdateStrategy::Always,
			};

			// Every series is a Japanese raw scan, so pages read right to left.
			manga.viewer = Viewer::RightToLeft;

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(parse_chapters(&html));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}/raw/{}/{}", manga.key, chapter.key);
		let html = Request::get(url)?.html()?;

		let server = defaults_get::<String>("imageServer").unwrap_or_else(|| String::from("1"));

		let pages = html
			.select("img.chapter-image")
			.map(|elements| {
				elements
					.filter_map(|element| {
						// data-original always holds the unproxied url, while src
						// may already have been rewritten for another server.
						let url = element
							.attr("data-original")
							.or_else(|| element.attr("abs:src"))?;
						Some(Page {
							content: PageContent::url(build_image_url(&server, &url)),
							..Default::default()
						})
					})
					.collect::<Vec<Page>>()
			})
			.unwrap_or_default();

		Ok(pages)
	}
}

impl MangaRawBest {
	fn fetch_manga_page(&self, query: &str, page: i32) -> Result<MangaPageResult> {
		let html = Request::get(format!("{BASE_URL}/manga-list?{query}"))?.html()?;
		Ok(parse_manga_page(&html, page))
	}
}

impl ListingProvider for MangaRawBest {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"views" => "-views",
			"views_week" => "-views_week",
			"created_at" => "-created_at",
			_ => SORT_VALUES[0],
		};

		let mut qs = QueryParameters::new();
		qs.push("sort", Some(sort));
		qs.push("page", Some(&page.to_string()));

		self.fetch_manga_page(&qs.to_string(), page)
	}
}

impl ImageRequestProvider for MangaRawBest {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for MangaRawBest {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = strip_base_url(&url).and_then(|path| path.strip_prefix("/raw/")) else {
			return Ok(None);
		};

		// Drop any query string or fragment a shared link may carry.
		let path = path
			.split(['?', '#'])
			.next()
			.unwrap_or(path)
			.trim_end_matches('/');
		let mut segments = path.split('/').filter(|segment| !segment.is_empty());

		let Some(manga_key) = segments.next() else {
			return Ok(None);
		};

		Ok(Some(match segments.next() {
			// ex: https://mangaraw.best/raw/tu-long-nobai/di-989hua
			Some(chapter_key) => DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: chapter_key.into(),
			},
			// ex: https://mangaraw.best/raw/tu-long-nobai
			None => DeepLinkResult::Manga {
				key: manga_key.into(),
			},
		}))
	}
}

register_source!(
	MangaRawBest,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test {
	use super::*;
	use aidoku::{ContentRating, alloc::vec};
	use aidoku_test::aidoku_test;

	const SERIES_KEY: &str = "tu-long-nobai";

	fn assert_valid_entries(entries: &[Manga]) {
		assert!(!entries.is_empty(), "no entries returned");

		for entry in entries {
			assert!(!entry.key.is_empty(), "entry has an empty key");
			assert!(
				!entry.key.contains('/'),
				"key should be a bare slug: {}",
				entry.key
			);
			assert!(!entry.title.is_empty(), "entry {} has no title", entry.key);

			let cover = entry.cover.as_deref().unwrap_or_default();
			assert!(
				cover.starts_with("http"),
				"entry {} has a non-absolute cover: {cover}",
				entry.key
			);
			assert!(
				!cover.contains("mangaraw-lazy"),
				"entry {} kept its placeholder cover",
				entry.key
			);
		}
	}

	#[aidoku_test]
	fn test_manga_list() {
		let source = MangaRawBest::new();
		let result = source
			.get_search_manga_list(None, 1, Vec::new())
			.expect("failed to fetch manga list");

		println!("entries: {}", result.entries.len());
		assert_valid_entries(&result.entries);
		assert!(result.has_next_page, "browse should be paginated");
	}

	#[aidoku_test]
	fn test_listings() {
		let source = MangaRawBest::new();

		for id in ["views", "views_week", "created_at"] {
			let result = source
				.get_manga_list(
					Listing {
						id: String::from(id),
						..Default::default()
					},
					1,
				)
				.unwrap_or_else(|_| panic!("failed to fetch listing {id}"));

			println!("{id}: {} entries", result.entries.len());
			assert_valid_entries(&result.entries);
		}
	}

	#[aidoku_test]
	fn test_search() {
		let source = MangaRawBest::new();
		let result = source
			.get_search_manga_list(Some(String::from("土竜")), 1, Vec::new())
			.expect("failed to search");

		println!("entries: {}", result.entries.len());
		assert_valid_entries(&result.entries);
		assert!(
			result.entries.iter().any(|manga| manga.key == SERIES_KEY),
			"search did not return the expected series"
		);
	}

	#[aidoku_test]
	fn test_search_without_results() {
		let source = MangaRawBest::new();
		let result = source
			.get_search_manga_list(Some(String::from("zzzqqqxxxnothing")), 1, Vec::new())
			.expect("failed to search");

		assert!(result.entries.is_empty());
		// A single (or empty) result page has no pagination bar, so the app must
		// not be told to request another page.
		assert!(!result.has_next_page);
	}

	#[aidoku_test]
	fn test_filters() {
		let source = MangaRawBest::new();
		let filtered = source
			.get_search_manga_list(
				None,
				1,
				vec![
					FilterValue::Sort {
						id: String::from("sort"),
						index: 5,
						ascending: false,
					},
					FilterValue::MultiSelect {
						id: String::from("status"),
						included: vec![String::from("2")],
						excluded: Vec::new(),
					},
					// 12 = アクション, 4 = ファンタジー, 10 = 成人向け
					FilterValue::MultiSelect {
						id: String::from("genre"),
						included: vec![String::from("12"), String::from("4")],
						excluded: vec![String::from("10")],
					},
				],
			)
			.expect("failed to fetch filtered list");

		println!("entries: {}", filtered.entries.len());
		assert_valid_entries(&filtered.entries);

		// The genre filter has to actually narrow the catalogue down, otherwise
		// the ids in filters.json are being silently ignored.
		let unfiltered = source
			.get_search_manga_list(None, 1, Vec::new())
			.expect("failed to fetch unfiltered list");
		assert_ne!(
			filtered.entries.first().map(|manga| &manga.key),
			unfiltered.entries.first().map(|manga| &manga.key),
			"filters had no effect on the results"
		);
	}

	#[aidoku_test]
	fn test_manga_details() {
		let source = MangaRawBest::new();
		let manga = source
			.get_manga_update(
				Manga {
					key: String::from(SERIES_KEY),
					..Default::default()
				},
				true,
				true,
			)
			.expect("failed to fetch details");

		println!("title: {}", manga.title);
		println!("cover: {:?}", manga.cover);
		println!("status: {:?}", manga.status);
		println!("rating: {:?}", manga.content_rating);

		assert_eq!(manga.title, "土竜の唄");
		assert_eq!(manga.status, MangaStatus::Ongoing);
		assert_eq!(manga.update_strategy, UpdateStrategy::Always);
		assert_eq!(manga.viewer, Viewer::RightToLeft);
		assert!(manga.cover.is_some_and(|cover| cover.starts_with("http")));
		assert!(manga.description.is_some_and(|text| !text.is_empty()));
		assert_eq!(
			manga.url.as_deref(),
			Some("https://mangaraw.best/raw/tu-long-nobai")
		);

		let chapters = manga.chapters.expect("no chapters returned");
		println!("chapters: {}", chapters.len());
		let first = chapters.first().expect("empty chapter list");
		println!(
			"first: {:?} key={} number={:?} date={:?}",
			first.title, first.key, first.chapter_number, first.date_uploaded
		);

		assert!(chapters.len() > 100, "suspiciously short chapter list");
		for chapter in &chapters {
			assert!(!chapter.key.is_empty(), "chapter has an empty key");
			assert!(
				!chapter.key.contains('/'),
				"chapter key should be a bare slug: {}",
				chapter.key
			);
			// "第N話" duplicates the number the app renders from chapter_number.
			assert!(
				chapter.title.is_none(),
				"redundant chapter title: {:?}",
				chapter.title
			);
			assert!(chapter.chapter_number.is_some());
			assert!(chapter.date_uploaded.is_some());
		}

		// Chapters are listed newest first.
		assert!(
			first.chapter_number >= chapters.last().and_then(|last| last.chapter_number),
			"chapters are not in descending order"
		);
	}

	#[aidoku_test]
	fn test_manga_tags_and_rating() {
		let source = MangaRawBest::new();
		let manga = source
			.get_manga_update(
				Manga {
					key: String::from("yao-wu-nohitorigotomao-mao-nohou-gong-mi-jie-kishou-zhang"),
					..Default::default()
				},
				true,
				false,
			)
			.expect("failed to fetch details");

		let tags = manga.tags.expect("expected genres on this series");
		println!("tags ({}): {:?}", tags.len(), tags);
		println!("rating: {:?}", manga.content_rating);

		assert!(tags.iter().any(|tag| tag == "Action"));
		assert!(tags.iter().any(|tag| tag == "Ecchi"));
		// The " raw" suffix the site appends to its Japanese genres is stripped.
		assert!(tags.iter().any(|tag| tag == "アクション"));

		let mut unique = tags.clone();
		unique.sort();
		unique.dedup();
		assert_eq!(unique.len(), tags.len(), "tags contain duplicates");

		// The SEO keyword cloud at the bottom of the page links to the same
		// genres but prefixes each label with the series title.
		for tag in &tags {
			assert!(
				!tag.contains(&manga.title),
				"tag picked up from the keyword cloud: {tag}"
			);
		}

		// This series is tagged Ecchi, so it must not be reported as safe.
		assert_eq!(manga.content_rating, ContentRating::Suggestive);
	}

	#[aidoku_test]
	fn test_page_list() {
		let source = MangaRawBest::new();
		let pages = source
			.get_page_list(
				Manga {
					key: String::from(SERIES_KEY),
					..Default::default()
				},
				Chapter {
					key: String::from("di-989hua"),
					..Default::default()
				},
			)
			.expect("failed to fetch pages");

		println!("pages: {}", pages.len());
		for page in pages.iter().take(3) {
			println!("  {:?}", page.content);
		}

		assert!(!pages.is_empty());
		for page in &pages {
			let PageContent::Url(url, _) = &page.content else {
				panic!("expected a url page");
			};
			assert!(url.starts_with("http"), "non-absolute page url: {url}");
		}
	}

	#[aidoku_test]
	fn test_deep_link() {
		let source = MangaRawBest::new();
		let handle = |url: &str| {
			source
				.handle_deep_link(String::from(url))
				.expect("failed to handle link")
		};

		assert!(matches!(
			handle("https://mangaraw.best/raw/tu-long-nobai"),
			Some(DeepLinkResult::Manga { ref key }) if key == SERIES_KEY
		));
		// Shared links may keep a trailing slash or tracking query.
		assert!(matches!(
			handle("http://www.mangaraw.best/raw/tu-long-nobai/?utm_source=x"),
			Some(DeepLinkResult::Manga { ref key }) if key == SERIES_KEY
		));
		assert!(matches!(
			handle("https://mangaraw.best/raw/tu-long-nobai/di-989hua"),
			Some(DeepLinkResult::Chapter { ref manga_key, ref key })
				if manga_key == SERIES_KEY && key == "di-989hua"
		));

		assert!(handle("https://mangaraw.best/manga-list").is_none());
		assert!(handle("https://example.com/raw/tu-long-nobai").is_none());
	}
}
