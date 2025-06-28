#![no_std]
use aidoku::{prelude::*, Source};
use liliana::{Impl, Liliana, Params};

const BASE_URL: &str = "https://raw1001.net";

struct Raw1001;

impl Impl for Raw1001 {
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

register_source!(Liliana<Raw1001>, ListingProvider, Home, DeepLinkHandler);
