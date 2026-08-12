#![expect(clippy::unwrap_used, clippy::panic)]

use super::*;
use aidoku::{
	Listing, ListingProvider, Manga, MangaPageResult, PageContent, Result,
	alloc::vec,
	imports::{
		defaults::{DefaultValue, defaults_set},
		std::sleep,
	},
};
use aidoku_test::aidoku_test;

<<<<<<< HEAD
/// "Solo Leveling", a completed webtoon with a few hundred chapters, used to check parsing
/// against.
const MANGA_KEY: &str = "/manga/solo-leveling";

/// The mirror listed second in `source.json`, reachable while the default url answers every
/// content path with a cloudflare challenge.
const MIRROR_URL: &str = "https://www.mangakakalove.com";

/// Selecting a base url is the whole point of the change, so the tests run against the value the
/// setting writes rather than the compiled in fallback. The test runner starts with no stored
/// defaults, so this stands in for the dropdown.
fn source() -> MangaBox<MangaKakalot> {
	pause();
=======
const MANGA_KEY: &str = "/manga/solo-leveling";

const MIRROR_URL: &str = "https://www.mangakakalove.com";

fn source() -> MangaBox<MangaKakalot> {
	sleep(3);
>>>>>>> 3ae3c93 (fix(en.mangakakalot): add minApiVersion, cleanup, add tests)
	defaults_set("url", DefaultValue::String(MIRROR_URL.into()));
	Source::new()
}

<<<<<<< HEAD
/// The site answers back to back requests with 429, and the test runner does not rate limit. Run
/// with `--test-threads=1` so these actually space the requests out.
fn pause() {
	sleep(3);
}

=======
>>>>>>> 3ae3c93 (fix(en.mangakakalot): add minApiVersion, cleanup, add tests)
fn listing(id: &str) -> Listing {
	Listing {
		id: id.into(),
		..Default::default()
	}
}

/// A 429 comes back as a page that parses into no entries at all, so give a request a couple of
/// extra tries before calling it a failure.
fn with_retry(mut fetch: impl FnMut() -> Result<MangaPageResult>) -> MangaPageResult {
	for attempt in 1..=3 {
		let result = fetch().expect("request failed");
		if !result.entries.is_empty() {
			return result;
		}
		assert!(attempt < 3, "no entries after {attempt} attempts");
		sleep(10);
	}
	unreachable!()
}

fn assert_entries(result: &MangaPageResult) {
	// the catalog spans thousands of pages, so the first one is never last
	assert!(result.has_next_page);

	let mut count = 0;
	for manga in &result.entries {
		// each page carries one hidden ad placeholder that matches the item selector and
		// comes through as an empty entry
		if manga.key == "#" {
			continue;
		}
		count += 1;

		assert!(
			manga.key.starts_with("/manga/"),
			"unexpected key {:?} (title {:?})",
			manga.key,
			manga.title
		);
		assert!(!manga.title.is_empty());
		assert!(
			manga
				.cover
				.as_deref()
				.is_some_and(|cover| cover.starts_with("https://"))
		);
	}

	assert!(count >= 10, "only {count} entries");
}

#[aidoku_test]
fn browse_test() {
	let source = source();
	let result = with_retry(|| source.get_search_manga_list(None, 1, vec![]));

	assert_entries(&result);
}

/// The listings are what broke: every one of them goes through `/genre/all?filter=`, which the
/// main url refuses with a cloudflare challenge while the home page keeps loading.
#[aidoku_test]
fn listings_test() {
	let source = source();

	for id in ["latest", "hot", "new", "completed"] {
		let result = with_retry(|| source.get_manga_list(listing(id), 1));

		assert_entries(&result);
<<<<<<< HEAD
		pause();
=======
		sleep(3);
>>>>>>> 3ae3c93 (fix(en.mangakakalot): add minApiVersion, cleanup, add tests)
	}
}

#[aidoku_test]
fn search_test() {
	let source = source();
	let result =
		with_retry(|| source.get_search_manga_list(Some("solo leveling".into()), 1, vec![]));

	assert!(result.entries.iter().any(|manga| manga.key == MANGA_KEY));
}

#[aidoku_test]
fn manga_details_test() {
	let manga = source()
		.get_manga_update(
			Manga {
				key: MANGA_KEY.into(),
				..Default::default()
			},
			true,
			true,
		)
		.expect("get_manga_update failed");

	assert_eq!(manga.title, "Solo Leveling");
	assert!(manga.cover.is_some());
	assert!(manga.description.is_some());
	assert!(
		manga
			.url
			.as_deref()
			.is_some_and(|url| url.ends_with(MANGA_KEY))
	);
<<<<<<< HEAD
	// authors, tags, status and viewer are read with `:contains`/`:containsOwn`, jsoup
	// extensions the test runner's css engine cannot parse, so they only fill in inside the
	// app and have to be checked there
=======
>>>>>>> 3ae3c93 (fix(en.mangakakalot): add minApiVersion, cleanup, add tests)

	let chapters = manga.chapters.expect("no chapters");
	// the chapter api pages 500 at a time, so this also covers a series past a single page
	assert!(chapters.len() >= 200);

	for chapter in &chapters {
		assert!(chapter.key.starts_with(MANGA_KEY));
		assert!(chapter.chapter_number.is_some());
		assert!(
			chapter
				.url
				.as_deref()
				.is_some_and(|url| url.starts_with("https://"))
		);
	}
}

#[aidoku_test]
fn page_list_test() {
	let source = source();
	let mut manga = source
		.get_manga_update(
			Manga {
				key: MANGA_KEY.into(),
				..Default::default()
			},
			false,
			true,
		)
		.expect("get_manga_update failed");
	let chapter = manga
		.chapters
		.take()
		.and_then(|chapters| chapters.into_iter().next())
		.expect("no chapters");
<<<<<<< HEAD
	pause();
=======
	sleep(3);
>>>>>>> 3ae3c93 (fix(en.mangakakalot): add minApiVersion, cleanup, add tests)

	let pages = source
		.get_page_list(manga, chapter)
		.expect("get_page_list failed");

	assert!(!pages.is_empty());
	for page in pages {
		match page.content {
			PageContent::Url(url, _) => assert!(url.starts_with("https://")),
			_ => panic!("expected a url page"),
		}
	}
}
