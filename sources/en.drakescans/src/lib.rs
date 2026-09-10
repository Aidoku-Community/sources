#![no_std]
use aidoku::{Source, prelude::*};
use vinetheme::{Impl, Params, VineTheme};

const BASE_URL: &str = "https://drakecomic.net";

struct DrakeScans;

impl Impl for DrakeScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(VineTheme<DrakeScans>, Home, DeepLinkHandler);
