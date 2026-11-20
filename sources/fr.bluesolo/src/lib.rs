#![no_std]
use aidoku::{Source, prelude::*};
use pizzareader::{Impl, Params, PizzaReader};

const BASE_URL: &str = "https://bluesolo.org";

struct BlueSolo;

impl Impl for BlueSolo {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(PizzaReader<BlueSolo>, DynamicFilters, DeepLinkHandler);
