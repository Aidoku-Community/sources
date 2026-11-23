#![no_std]
use aidoku::{Source, prelude::*};
use toonz::{Impl, Params, Row, Toonz};

const BASE_URL: &str = "https://toonx.to";

struct Toonx;

impl Impl for Toonx {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL,
			catalog: "comics",
			series_path: "comic",
			// `/comic/` redirects to `/adult/`.
			series_types: &["comic", "adult"],
			// Every title is an explicit comic, so the catalogue has no lower rating or other type.
			rating: "pornographic",
			type_count: 0,
			home_pages: &["/", "/comics?sort=latest&view=list"],
			home_rows: &[
				("Top Rated This Month", Row::Featured(&[0])),
				("New Titles", Row::Newest(&[0], "New Titles")),
				("Recently Updated", Row::Updates(1)),
				("Trending", Row::Tab(0, "Trending")),
				("Popular Updates", Row::Tab(0, "Popular Updates")),
			],
		}
	}
}

register_source!(
	Toonz<Toonx>,
	Home,
	ListingProvider,
	DynamicFilters,
	DynamicSettings,
	DeepLinkHandler
);
