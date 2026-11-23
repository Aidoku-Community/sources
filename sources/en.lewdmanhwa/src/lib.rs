#![no_std]
use aidoku::{Source, prelude::*};
use toonz::{Impl, Params, Row, Toonz};

const BASE_URL: &str = "https://lewdmanhwa.com";

struct LewdManhwa;

impl Impl for LewdManhwa {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL,
			catalog: "webtoons",
			series_path: "webtoon",
			series_types: &["webtoon"],
			rating: "pornographic",
			type_count: 5,
			home_pages: &["/home"],
			home_rows: &[
				("Featured", Row::Hero(0)),
				("Latest Updates", Row::Chapters(0, "Latest Updates")),
			],
		}
	}
}

register_source!(
	Toonz<LewdManhwa>,
	Home,
	ListingProvider,
	DynamicFilters,
	DynamicSettings,
	DeepLinkHandler
);
