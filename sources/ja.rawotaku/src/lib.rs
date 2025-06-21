#![no_std]
use aidoku::{alloc::borrow::Cow, prelude::*, Source};
use mangareader::{Impl, MangaReader, Params};

const BASE_URL: &str = "https://rawotaku.com";

struct RawOtaku;

impl Impl for RawOtaku {
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
	MangaReader<RawOtaku>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
