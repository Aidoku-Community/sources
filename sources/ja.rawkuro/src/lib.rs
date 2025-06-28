#![no_std]
use aidoku::{prelude::*, Source};
use liliana::{Impl, Liliana, Params};

const BASE_URL: &str = "https://rawkuro.net";

struct RawKuro;

impl Impl for RawKuro {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			..Default::default()
		}
	}
}

register_source!(Liliana<RawKuro>, ListingProvider, Home, DeepLinkHandler);
