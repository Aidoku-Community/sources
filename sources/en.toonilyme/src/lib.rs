#![no_std]
use aidoku::{Source, prelude::*};
use mangak::{Impl, MangaK, Params};

const BASE_URL: &str = "https://toontop.io";
const API_URL: &str = "https://api.toontop.io";

struct ToonTop;

impl Impl for ToonTop {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
			..Default::default()
		}
	}
}

register_source!(
	MangaK<ToonTop>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test {
	use super::*;
	use aidoku::{
		DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponentValue, Listing,
		ListingProvider, Manga, MangaStatus,
		alloc::{Vec, vec},
	};
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn search_with_filters() {
		let result = MangaK::<ToonTop>::new()
			.get_search_manga_list(
				Some("love".into()),
				1,
				vec![
					FilterValue::MultiSelect {
						id: "genres".into(),
						included: vec!["romance".into()],
						excluded: vec![],
					},
					FilterValue::Check {
						id: "is_uncensored".into(),
						value: 1,
					},
				],
			)
			.expect("search failed");
		assert!(!result.entries.is_empty());
	}

	#[aidoku_test]
	fn raw_listing() {
		let result = MangaK::<ToonTop>::new()
			.get_manga_list(
				Listing {
					id: "raw".into(),
					..Default::default()
				},
				1,
			)
			.expect("listing failed");
		assert!(result.entries.iter().all(|m| m.title.ends_with("Raw")));
		assert!(result.has_next_page);
	}

	#[aidoku_test]
	fn details_and_pages() {
		let source = MangaK::<ToonTop>::new();
		let manga = source
			.get_manga_update(
				Manga {
					key: "/magic-emperor".into(),
					..Default::default()
				},
				true,
				true,
			)
			.expect("update failed");
		assert_eq!(manga.title, "Magic Emperor");
		assert_eq!(manga.status, MangaStatus::Ongoing);
		let chapters = manga.chapters.clone().expect("chapters missing");
		assert!(chapters.len() > 50);
		assert!(chapters[0].chapter_number.is_some());
		// date_uploaded isn't checked since the test runner's parse_date doesn't support quoted literals

		let pages = source
			.get_page_list(manga, chapters[0].clone())
			.expect("pages failed");
		assert!(!pages.is_empty());
	}

	#[aidoku_test]
	fn home_sections() {
		let home = MangaK::<ToonTop>::new().get_home().expect("home failed");
		let titles = home
			.components
			.iter()
			.map(|component| component.title.as_deref().unwrap_or_default())
			.collect::<Vec<_>>();
		assert_eq!(
			titles,
			[
				"Top This Week",
				"Trending This Week",
				"Popular Updates",
				"New Manga",
				"Latest Updates"
			]
		);
		let HomeComponentValue::BigScroller { entries, .. } = &home.components[0].value else {
			panic!("top this week is not a big scroller");
		};
		assert!(entries.iter().all(|manga| manga.tags.is_some()));
		let HomeComponentValue::MangaChapterList { entries, .. } = &home.components[4].value else {
			panic!("latest updates is not a chapter list");
		};
		assert!(!entries.is_empty());
	}

	#[aidoku_test]
	fn deep_link() {
		let result = MangaK::<ToonTop>::new()
			.handle_deep_link("https://toontop.io/magic-emperor/chapter-925".into())
			.expect("deep link failed");
		assert!(matches!(
			result,
			Some(DeepLinkResult::Chapter { manga_key, key })
				if manga_key == "/magic-emperor" && key == "/magic-emperor/chapter-925"
		));
	}
}
