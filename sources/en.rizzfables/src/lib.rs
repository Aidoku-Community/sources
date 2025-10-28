#![no_std]
use aidoku::{Source, alloc::string::String, imports::html::Document, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params, helpers};

const BASE_URL: &str = "https://rizzfables.com";

struct RizzFables;

impl Impl for RizzFables {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			manga_url_directory: "/series".into(),
			date_format: "dd MMM yyyy".into(),
			..Default::default()
		}
	}

	fn parse_description(&self, _params: &Params, html: &Document) -> Option<String> {
		html.select_first(".entry-content-single > script")
			.and_then(|el| el.data())
			.and_then(|data| {
				helpers::extract_between(&data, r#"var description = ""#, r#"" ;"#).map(|s| {
					s.replace("\\u201d", "\"")
						.replace("\\u201c", "\"")
						.replace("\\u2014", "—")
						.replace("\\u2019", "'")
						.replace("\\u2026", "…")
						.replace("\\r\\n", "\n")
						.replace("\\n", "\n")
						.replace("\\\"", "\"")
				})
			})
	}
}

register_source!(
	MangaThemesia<RizzFables>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
