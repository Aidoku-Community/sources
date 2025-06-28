#![no_std]
use aidoku::{prelude::*, Source};
use liliana::{Impl, Liliana, Params};

const BASE_URL: &str = "https://mangasect.net";

struct MangaSect;

impl Impl for MangaSect {
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

register_source!(Liliana<MangaSect>, ListingProvider, Home, DeepLinkHandler);
