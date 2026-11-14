#![no_std]
use aidoku::{HomeLayout, Result, Source, prelude::*};
use vinetheme::{Impl, Params, VineTheme};

const BASE_URL: &str = "https://drakecomic.net";

struct DrakeScans;

impl Impl for DrakeScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}

	fn parse_home(&self, params: &Params, body: &str) -> Result<HomeLayout> {
		let mut home = vinetheme::home::parse(body, params)?;
		if let Some(index) = home
			.components
			.iter()
			.position(|c| c.title.as_deref() == Some("Popular Today"))
		{
			let popular = home.components.remove(index);
			let index = home
				.components
				.iter()
				.position(|c| c.title.as_deref() == Some("Latest Novel Updates"))
				.map_or(home.components.len(), |index| index + 1);
			home.components.insert(index, popular);
		}
		Ok(home)
	}
}

register_source!(
	VineTheme<DrakeScans>,
	Home,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler
);
