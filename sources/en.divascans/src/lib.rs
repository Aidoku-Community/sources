#![no_std]
use aidoku::{HomeLayout, Result, Source, prelude::*};
use vinetheme::{Impl, Params, VineTheme};

const BASE_URL: &str = "https://divascans.org";

struct DivaScans;

impl Impl for DivaScans {
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
				("Featured", None),
				("Popular Today", None),
				("Latest Comic Updates", Some(4)),
				("Latest Novel Updates", Some(4)),
				("New Series", None),
				("Top 10 in All", Some(3)),
			],
		))
	}
}

register_source!(
	VineTheme<DivaScans>,
	Home,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler
);
