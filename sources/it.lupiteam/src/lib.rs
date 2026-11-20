#![no_std]
use aidoku::{Source, prelude::*};
use pizzareader::{Impl, Params, PizzaReader};

const BASE_URL: &str = "https://lupiteam.net";

struct LupiTeam;

impl Impl for LupiTeam {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(PizzaReader<LupiTeam>, DynamicFilters, DeepLinkHandler);
