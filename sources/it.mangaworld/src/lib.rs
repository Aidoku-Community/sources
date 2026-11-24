#![no_std]
use aidoku::{Source, prelude::*};
use mangaworld_template::{Impl, MangaWorldTemplate, Params};

const BASE_URL: &str = "https://www.mangaworld.me";

struct MangaWorld;

impl Impl for MangaWorld {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(
	MangaWorldTemplate<MangaWorld>,
	DeepLinkHandler,
	ListingProvider,
	Home
);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::{
		FilterValue, Home, HomeComponentValue, Listing, ListingProvider,
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
		let HomeComponentValue::MangaChapterList { entries, .. } = &latest.value else {
			panic!("latest section should use the native chapter list");
		};
		assert!(!entries.is_empty(), "latest chapter list is empty");
		assert!(entries.iter().all(|entry| {
			entry
				.chapter
				.title
				.as_deref()
				.is_some_and(|title| title.contains("capitoli totali"))
		}));
		assert!(
			entries
				.iter()
				.any(|entry| entry.chapter.date_uploaded.is_some()),
			"expected publication dates for recent chapters"
		);
	}

	#[aidoku_test]
	fn chapter_bounds_filter_search() {
		let source = MangaWorldTemplate::<MangaWorld>::new();
		let filtered = source
			.get_search_manga_list(
				Some("One Piece".into()),
				1,
				Vec::from([FilterValue::Text {
					id: "min_chapters".into(),
					value: "100".into(),
				}]),
			)
			.expect("minimum chapter filter should work");
		assert!(
			filtered
				.entries
				.iter()
				.any(|m| m.title.eq_ignore_ascii_case("One Piece"))
		);
		assert!(
			filtered
				.entries
				.iter()
				.all(|m| m.chapters.as_ref().is_some_and(|c| c.len() >= 100))
		);

		let filtered = source
			.get_search_manga_list(
				Some("One Piece".into()),
				1,
				Vec::from([FilterValue::Text {
					id: "max_chapters".into(),
					value: "10".into(),
				}]),
			)
			.expect("maximum chapter filter should work");
		assert!(
			!filtered
				.entries
				.iter()
				.any(|m| m.title.eq_ignore_ascii_case("One Piece"))
		);
		assert!(
			filtered
				.entries
				.iter()
				.all(|m| m.chapters.as_ref().is_some_and(|c| c.len() <= 10))
		);
	}

	#[aidoku_test]
	fn chapter_bounds_reject_invalid_range() {
		let source = MangaWorldTemplate::<MangaWorld>::new();
		let result = source.get_search_manga_list(
			None,
			1,
			Vec::from([
				FilterValue::Text {
					id: "min_chapters".into(),
					value: "100".into(),
				},
				FilterValue::Text {
					id: "max_chapters".into(),
					value: "10".into(),
				},
			]),
		);
		assert!(result.is_err());
	}
}
