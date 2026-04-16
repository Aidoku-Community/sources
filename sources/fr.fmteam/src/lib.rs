#![no_std]
use aidoku::{Source, prelude::*};
use pizzareader::{Impl, Params, PizzaReader};

const BASE_URL: &str = "https://fmteam.fr";

struct Fmteam;

impl Impl for Fmteam {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(PizzaReader<Fmteam>, DeepLinkHandler, Home);
