#![no_std]
use aidoku::{HomeLayout, Result, Source, prelude::*};

use vinetheme::{Impl, Params, VineTheme};

mod home;

const BASE_URL: &str = "https://valirscans.org";

struct ValirScans;

impl Impl for ValirScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		home::get_home(params)
	}
}

register_source!(
	VineTheme<ValirScans>,
	Home,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler
);
