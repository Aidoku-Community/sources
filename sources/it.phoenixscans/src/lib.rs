#![no_std]
use aidoku::{Source, prelude::*};
use pizzareader::{Impl, Params, PizzaReader};

const BASE_URL: &str = "https://www.phoenixscans.com";

struct PhoenixScans;

impl Impl for PhoenixScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(PizzaReader<PhoenixScans>, DynamicFilters, DeepLinkHandler);
