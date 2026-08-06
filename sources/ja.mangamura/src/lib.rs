#![no_std]
use aidoku::{Source, alloc::borrow::Cow, prelude::*};
use mangareader::{Impl, MangaReader, Params};

const BASE_URL: &str = "https://mangamura.me";

struct MangaMura;

impl Impl for MangaMura {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			search_path: "".into(),
			search_param: "q".into(),
			page_param: "p".into(),
			get_chapter_selector: || "#ja-chaps > li".into(),
			get_chapter_language: |_| "ja".into(),
			get_page_url_path: |chapter_id| format!("/json/chapter?id={chapter_id}&mode=vertical"),
			set_default_filters: |query_params| {
				query_params.set("type", Some("all"));
				query_params.set("status", Some("all"));
				query_params.set("language", Some("all"));
				query_params.set("sort", Some("default"));
			},
			..Default::default()
		}
	}

	fn get_sort_id(&self, index: i32) -> Cow<'static, str> {
		match index {
			0 => "default",
			1 => "latest-update",
			2 => "most-viewed",
			3 => "title-az",
			4 => "title-za",
			_ => "default",
		}
		.into()
	}
}

register_source!(
	MangaReader<MangaMura>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test {
	use super::*;
	use aidoku::alloc::{String, vec::Vec};
	use aidoku_test::aidoku_test;

	fn source() -> MangaReader<MangaMura> {
		Source::new()
	}

	// The site links entries with absolute urls, so keys are only stripped down
	// to paths when BASE_URL matches the live domain. A stale domain silently
	// turns every key into a full url and breaks details and chapter lists.
	#[aidoku_test]
	fn search_returns_path_keys() {
		let result = source()
			.get_search_manga_list(Some(String::from("ワンピース")), 1, Vec::new())
			.expect("search failed");
		assert!(!result.entries.is_empty(), "expected at least one result");
		for manga in &result.entries {
			assert!(
				manga.key.starts_with('/'),
				"expected a path key, got {}",
				manga.key
			);
		}
	}

	#[aidoku_test]
	fn manga_details_have_chapters() {
		let source = source();
		let manga = source
			.get_search_manga_list(Some(String::from("ワンピース")), 1, Vec::new())
			.expect("search failed")
			.entries
			.into_iter()
			.next()
			.expect("expected at least one result");
		let manga = source
			.get_manga_update(manga, true, true)
			.expect("get_manga_update failed");
		let chapters = manga.chapters.expect("no chapters returned");
		assert!(chapters.len() > 100, "got {} chapters", chapters.len());
	}
}
