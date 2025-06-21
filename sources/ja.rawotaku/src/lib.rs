#![no_std]
use aidoku::{
	alloc::{borrow::Cow, String},
	helpers::uri::QueryParameters,
	imports::html::Element,
	prelude::*,
	Source,
};
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
			..Default::default()
		}
	}

	fn get_chapter_selector(&self) -> Cow<'static, str> {
		"#ja-chaps > li".into()
	}

	fn get_chapter_language(&self, _element: &Element) -> String {
		"ja".into()
	}

	fn get_page_url_path(&self, chapter_id: &str) -> String {
		format!("/json/chapter?id={chapter_id}&mode=vertical")
	}

	fn set_default_filters(&self, query_params: &mut QueryParameters) {
		query_params.set("type", Some("all"));
		query_params.set("status", Some("all"));
		query_params.set("language", Some("all"));
		query_params.set("sort", Some("default"));
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
