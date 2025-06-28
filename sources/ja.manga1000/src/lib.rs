#![no_std]
use aidoku::{prelude::*, Source};
use liliana::{Impl, Liliana, Params};

const BASE_URL: &str = "https://manga1000.top";

struct Manga1000;

impl Impl for Manga1000 {
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

register_source!(Liliana<Manga1000>, ListingProvider, Home, DeepLinkHandler);
