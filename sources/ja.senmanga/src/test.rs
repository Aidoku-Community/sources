use super::*;
use aidoku::alloc::vec;
use aidoku_test::aidoku_test;

/// A long-running series, so its chapter list stays large enough to assert on.
const TEST_MANGA_KEY: &str = "one-piece";

#[aidoku_test]
fn browse_test() {
	let source = SenManga;
	let result = source
		.get_search_manga_list(None, 1, vec![])
		.expect("get_search_manga_list failed");

	assert!(result.entries.len() >= 10);
	// The directory spans hundreds of pages, so the first one is never last.
	assert!(result.has_next_page);

	for manga in result.entries {
		assert!(!manga.key.is_empty());
		assert!(!manga.title.is_empty());
		assert!(
			manga
				.cover
				.as_deref()
				.is_some_and(|cover| cover.starts_with("https://"))
		);
	}
}

#[aidoku_test]
fn search_test() {
	let source = SenManga;

	// The api matches against alternative titles too, so a japanese query
	// has to reach the romanized entry.
	let result = source
		.get_search_manga_list(Some("ワンピース".into()), 1, vec![])
		.expect("get_search_manga_list failed");

	assert!(!result.entries.is_empty());
	assert!(
		result
			.entries
			.iter()
			.any(|manga| manga.key == TEST_MANGA_KEY)
	);
}

#[aidoku_test]
fn manga_details_test() {
	let source = SenManga;
	let manga = source
		.get_manga_update(
			Manga {
				key: TEST_MANGA_KEY.into(),
				..Default::default()
			},
			true,
			true,
		)
		.expect("get_manga_update failed");

	assert!(!manga.title.is_empty());
	assert!(manga.cover.is_some());
	assert!(manga.description.is_some());
	assert!(manga.tags.as_ref().is_some_and(|tags| !tags.is_empty()));
	assert_eq!(manga.viewer, Viewer::RightToLeft);

	let chapters = manga.chapters.expect("no chapters");
	assert!(chapters.len() >= 100);

	for chapter in chapters {
		assert!(!chapter.key.is_empty());
		assert!(chapter.chapter_number.is_some());
		// Setting a language would hide every chapter behind the app's
		// language filter, since this source is japanese-only.
		assert!(chapter.language.is_none());
	}
}

#[aidoku_test]
fn status_test() {
	let source = SenManga;

	// The listing carries a status, the details endpoint answers with a
	// null one, so what the manga arrives with has to survive the fetch.
	let result = source
		.get_search_manga_list(Some("ワンピース".into()), 1, vec![])
		.expect("get_search_manga_list failed");
	let listed = result
		.entries
		.into_iter()
		.find(|manga| manga.key == TEST_MANGA_KEY)
		.expect("test manga missing from search results");
	assert_eq!(listed.status, MangaStatus::Ongoing);

	let manga = source
		.get_manga_update(listed, true, false)
		.expect("get_manga_update failed");
	assert_eq!(manga.status, MangaStatus::Ongoing);
}

#[aidoku_test]
fn page_list_test() {
	let source = SenManga;
	let manga = source
		.get_manga_update(
			Manga {
				key: TEST_MANGA_KEY.into(),
				..Default::default()
			},
			false,
			true,
		)
		.expect("get_manga_update failed");
	let chapter = manga
		.chapters
		.as_ref()
		.and_then(|chapters| chapters.first())
		.expect("no chapters")
		.clone();

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

#[aidoku_test]
fn deep_link_test() {
	let source = SenManga;

	let result = source
		.handle_deep_link(format!("{BASE_URL}/manga/{TEST_MANGA_KEY}/"))
		.expect("handle_deep_link failed");
	assert_eq!(
		result,
		Some(DeepLinkResult::Manga {
			key: TEST_MANGA_KEY.into()
		})
	);

	let result = source
		.handle_deep_link(format!(
			"{BASE_URL}/manga/{TEST_MANGA_KEY}/chapter-8.338323/"
		))
		.expect("handle_deep_link failed");
	assert_eq!(
		result,
		Some(DeepLinkResult::Chapter {
			manga_key: TEST_MANGA_KEY.into(),
			key: "8.338323".into()
		})
	);

	let result = source
		.handle_deep_link(format!("{BASE_URL}/directory"))
		.expect("handle_deep_link failed");
	assert_eq!(result, None);
}
