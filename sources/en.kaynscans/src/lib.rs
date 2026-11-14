#![no_std]
use aidoku::{HomeLayout, Result, Source, prelude::*};
use vinetheme::{Impl, Params, VineTheme};

const BASE_URL: &str = "https://kaynscans.com";

struct KaynScans;

impl Impl for KaynScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}

	fn parse_home(&self, params: &Params, body: &str) -> Result<HomeLayout> {
		Ok(vinetheme::home::reorder(
			vinetheme::home::parse(body, params)?,
			&[
				("Editor's Pick", None),
				("Popular Today", None),
				("Latest Comic Updates", Some(4)),
				("New Series", None),
				("Latest Novel Updates", Some(4)),
				("Top 10 in All", Some(3)),
			],
		))
	}
}

register_source!(
	VineTheme<KaynScans>,
	Home,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler
);
