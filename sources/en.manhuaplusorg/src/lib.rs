#![no_std]
use aidoku::{prelude::*, Source};
use liliana::{Impl, Liliana, Params};

const BASE_URL: &str = "https://manhuaplus.org";

struct ManhuaplusOrg;

impl Impl for ManhuaplusOrg {
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
	Liliana<ManhuaplusOrg>,
	ListingProvider,
	Home,
	DeepLinkHandler
);
