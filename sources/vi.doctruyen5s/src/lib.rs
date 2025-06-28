#![no_std]
use aidoku::{prelude::*, Source};
use liliana::{Impl, Liliana, Params};

const BASE_URL: &str = "https://dongmoe.com";

struct DocTruyen5s;

impl Impl for DocTruyen5s {
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

register_source!(
	Liliana<DocTruyen5s>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
