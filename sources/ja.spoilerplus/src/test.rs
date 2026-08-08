use super::*;
use aidoku::{DeepLinkHandler, MangaPageResult, alloc::vec};
use aidoku_test::aidoku_test;

const SORT_UPDATED: i32 = 0;
const SORT_RANKING: i32 = 1;

const SERIES_KEY: &str = "/HUNTER X HUNTER-raw-free/";

fn source() -> WpComics<SpoilerPlus> {
	WpComics::new()
}

fn sort(index: i32) -> Vec<FilterValue> {
	vec![FilterValue::Sort {
		id: String::from("sort"),
		index,
		ascending: false,
	}]
}

fn browse(index: i32, page: i32) -> MangaPageResult {
	source()
		.get_search_manga_list(None, page, sort(index))
		.expect("browse request should succeed")
}

fn series() -> Manga {
	source()
		.get_manga_update(
			Manga {
				key: SERIES_KEY.into(),
				..Default::default()
			},
			true,
			true,
		)
		.expect("details request should succeed")
}

fn leading_keys(result: &MangaPageResult) -> Vec<&String> {
	result
		.entries
		.iter()
		.take(5)
		.map(|manga| &manga.key)
		.collect()
}

// The app sends no filter value until one is picked, so the fallback has to be
// a real ordering rather than an out of range option.
#[aidoku_test]
fn test_sort_index_falls_back_to_the_first_option() {
	assert_eq!(sort_index(&[]), SORT_UPDATED);
	assert_eq!(sort_index(&sort(SORT_RANKING)), SORT_RANKING);
}

#[aidoku_test]
fn test_sort_updated() {
	assert!(
		!browse(SORT_UPDATED, 1).entries.is_empty(),
		"updates ordering should return entries"
	);
}

#[aidoku_test]
fn test_sort_updated_page_2() {
	assert!(
		!browse(SORT_UPDATED, 2).entries.is_empty(),
		"updates ordering page 2 should return entries"
	);
}

#[aidoku_test]
fn test_sort_ranking() {
	assert!(
		!browse(SORT_RANKING, 1).entries.is_empty(),
		"ranking ordering should return entries"
	);
}

#[aidoku_test]
fn test_browse_without_filters() {
	let result = source()
		.get_search_manga_list(None, 1, Vec::new())
		.expect("browse request should succeed");
	assert!(!result.entries.is_empty(), "browse should return entries");
}

#[aidoku_test]
fn test_search() {
	let result = source()
		.get_search_manga_list(Some(String::from("ワンピース")), 1, Vec::new())
		.expect("search request should succeed");
	assert!(!result.entries.is_empty(), "search should return entries");
}

#[aidoku_test]
fn test_query_takes_precedence_over_sort() {
	let searched = source()
		.get_search_manga_list(Some(String::from("ワンピース")), 1, sort(SORT_RANKING))
		.expect("search request should succeed");
	let ranking = browse(SORT_RANKING, 1);
	assert!(!searched.entries.is_empty(), "search should return entries");
	assert_ne!(
		leading_keys(&searched),
		leading_keys(&ranking),
		"a query should search rather than fall back to the ranking path"
	);
}

// The updates ordering starts at the site home page, which stacks a carousel
// and a ranking block around the paginated list.
#[aidoku_test]
fn test_updated_page_1_holds_only_the_paginated_block() {
	let result = browse(SORT_UPDATED, 1);

	let mut keys = result
		.entries
		.iter()
		.map(|manga| manga.key.as_str())
		.collect::<Vec<_>>();
	let total = keys.len();
	keys.sort_unstable();
	keys.dedup();
	assert_eq!(keys.len(), total, "a page should not repeat entries");

	// one block currently holds 24 entries
	assert!(
		total <= 40,
		"a page should hold a single block, got {total} entries"
	);
}

#[aidoku_test]
fn test_keys_stay_relative_and_covers_absolute() {
	let result = browse(SORT_UPDATED, 1);

	for manga in &result.entries {
		assert!(
			manga.key.starts_with('/') && manga.key.ends_with("-raw-free/"),
			"key should be a site-relative series path, got {}",
			manga.key
		);
		let cover = manga.cover.as_ref().expect("entry should have a cover");
		assert!(
			cover.starts_with(BASE_URL),
			"cover should be an absolute url, got {cover}"
		);
	}
}

#[aidoku_test]
fn test_pagination_ends() {
	let result = browse(SORT_UPDATED, 9999);
	assert!(
		result.entries.is_empty() && !result.has_next_page,
		"out of range page should end pagination"
	);
}

#[aidoku_test]
fn test_sorts_return_different_orders() {
	let updated = browse(SORT_UPDATED, 1);
	let ranking = browse(SORT_RANKING, 1);
	assert_ne!(
		leading_keys(&updated),
		leading_keys(&ranking),
		"the sort options should not resolve to the same order"
	);
}

#[aidoku_test]
fn test_manga_details() {
	let manga = series();
	assert_eq!(
		manga.title, "HUNTER X HUNTER",
		"title should drop the suffix"
	);
	let cover = manga.cover.expect("series should have a cover");
	assert!(
		cover.starts_with(BASE_URL),
		"cover should be an absolute url, got {cover}"
	);
	assert!(
		manga.description.is_some_and(|d| !d.is_empty()),
		"series should have a description"
	);
	assert!(
		manga.tags.is_some_and(|t| !t.is_empty()),
		"series should have tags"
	);
}

#[aidoku_test]
fn test_status_is_not_read_from_the_alternative_titles_row() {
	assert_eq!(
		series().status,
		MangaStatus::Ongoing,
		"a running series should not be read off the alternative titles row"
	);
}

#[aidoku_test]
fn test_chapter_list() {
	let chapters = series().chapters.expect("series should have chapters");
	assert!(chapters.len() >= 400, "got {} chapters", chapters.len());

	let first = chapters.first().expect("chapter list should not be empty");
	assert!(
		first.key.starts_with(SERIES_KEY),
		"chapter key should sit under the series path, got {}",
		first.key
	);
	assert!(
		first.chapter_number.is_some(),
		"chapter number should be parsed out of the title"
	);
}

#[aidoku_test]
fn test_page_list() {
	let chapters = series().chapters.expect("series should have chapters");
	let chapter = chapters
		.into_iter()
		.next()
		.expect("chapter list should not be empty");
	let pages = source()
		.get_page_list(Manga::default(), chapter)
		.expect("page list request should succeed");

	assert!(!pages.is_empty(), "chapter should have pages");
	for page in &pages {
		let PageContent::Url(url, context) = &page.content else {
			panic!("page should be a url");
		};
		assert!(
			url.starts_with(IMG_CDN),
			"page should be served from the image cdn, got {url}"
		);
		// the descrambling order travels with the page, so a missing key would
		// silently ship scrambled images
		assert!(
			context
				.as_ref()
				.and_then(|c| c.get("key"))
				.is_some_and(|key| !key.is_empty()),
			"page should carry the descrambling key"
		);
	}
}

#[aidoku_test]
fn test_deep_links() {
	let handle = |url: &str| {
		source()
			.handle_deep_link(String::from(url))
			.expect("deep link should be handled")
	};

	assert_eq!(
		handle("https://spoilerplus.tv/HUNTER%20X%20HUNTER-raw-free/"),
		Some(DeepLinkResult::Manga {
			key: SERIES_KEY.into()
		})
	);
	assert_eq!(
		handle("https://spoilerplus.tv/HUNTER%20X%20HUNTER-raw-free/第417話/"),
		Some(DeepLinkResult::Chapter {
			manga_key: SERIES_KEY.into(),
			key: "/HUNTER X HUNTER-raw-free/第417話/".into(),
		})
	);
	assert_eq!(handle("https://spoilerplus.tv/genre/Ecchi/"), None);
	assert_eq!(handle("https://spoilerplus.tv/ranking/"), None);
	assert_eq!(handle("https://example.com/HUNTER-raw-free/"), None);
}
