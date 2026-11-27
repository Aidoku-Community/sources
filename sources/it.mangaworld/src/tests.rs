use super::*;
use aidoku::{
	Home, HomeComponentValue, Listing, ListingProvider,
	alloc::{String, Vec},
};
use aidoku_test::aidoku_test;

#[aidoku_test]
fn catalog_returns_entries() {
	let source = MangaWorldTemplate::<MangaWorld>::new();
	let result = source
		.get_search_manga_list(None, 1, Vec::new())
		.expect("catalog request should succeed");
	assert!(!result.entries.is_empty(), "catalog returned no manga");
	assert!(
		result
			.entries
			.iter()
			.all(|m| !m.title.is_empty() && m.key.starts_with("/manga/"))
	);
}

#[aidoku_test]
fn search_details_chapters_and_reader() {
	let source = MangaWorldTemplate::<MangaWorld>::new();
	let result = source
		.get_search_manga_list(Some(String::from("One Piece")), 1, Vec::new())
		.expect("search request should succeed");
	let manga = result
		.entries
		.into_iter()
		.find(|m| m.title.eq_ignore_ascii_case("One Piece"))
		.expect("search should return One Piece");
	let mut manga = source
		.get_manga_update(manga, true, true)
		.expect("details and chapters request should succeed");
	assert!(!manga.title.is_empty());
	assert!(manga.cover.is_some(), "missing cover");
	let chapter = manga
		.chapters
		.take()
		.expect("missing chapter list")
		.into_iter()
		.next()
		.expect("chapter list is empty");
	let pages = source
		.get_page_list(manga, chapter)
		.expect("reader request should succeed");
	assert!(!pages.is_empty(), "reader returned no pages");
}

#[aidoku_test]
fn latest_updates_returns_entries() {
	let source = MangaWorldTemplate::<MangaWorld>::new();
	let result = source
		.get_manga_list(
			Listing {
				id: "latest".into(),
				name: "Ultimi aggiornamenti".into(),
				..Default::default()
			},
			1,
		)
		.expect("latest updates request should succeed");
	assert!(!result.entries.is_empty(), "homepage returned no updates");
}

#[aidoku_test]
fn home_returns_sections() {
	let source = MangaWorldTemplate::<MangaWorld>::new();
	let home = source.get_home().expect("home should load");
	assert!(
		home.components.len() >= 3,
		"expected latest, new, and popular lists"
	);
	let latest = home.components.first().expect("missing latest section");
	let HomeComponentValue::Scroller { entries, .. } = &latest.value else {
		panic!("latest section should use a scroller");
	};
	assert!(!entries.is_empty(), "latest section is empty");
}

