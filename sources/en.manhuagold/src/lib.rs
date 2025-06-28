#![no_std]
use aidoku::{prelude::*, Source};
use liliana::{Impl, Liliana, Params};

const BASE_URL: &str = "https://manhuagold.top";

struct Manhuagold;

impl Impl for Manhuagold {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			uses_post_search: true,
		}
	}
}

register_source!(Liliana<Manhuagold>, ListingProvider, Home, DeepLinkHandler);
