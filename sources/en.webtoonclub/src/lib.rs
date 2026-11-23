#![no_std]
use aidoku::{Page, Result, Source, alloc::Vec, prelude::*};
use toonz::{Impl, Params, Row, Toonz};

mod helpers;
mod models;

const BASE_URL: &str = "https://webtoonclub.com";

struct WebtoonClub;

impl Impl for WebtoonClub {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL,
			catalog: "webtoons",
			series_path: "webtoon",
			series_types: &["webtoon"],
			rating: "suggestive",
			type_count: 5,
			home_pages: &["/", "/webtoons?sort=latest&view=list"],
			home_rows: &[
				("Top Rated This Month", Row::Featured(&[0])),
				("New Titles", Row::Newest(&[0], "New Titles")),
				("Recently Updated", Row::Updates(1)),
				("Trending", Row::Tab(0, "Trending")),
				("Popular Updates", Row::Tab(0, "Popular Updates")),
			],
		}
	}

	fn parse_page_list(&self, _params: &Params, body: &str) -> Result<Vec<Page>> {
		helpers::parse_page_list(body)
	}
}

register_source!(
	Toonz<WebtoonClub>,
	Home,
	ListingProvider,
	DynamicFilters,
	DynamicSettings,
	DeepLinkHandler
);
