#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{Impl, Keyoapp, Params};

const BASE_URL: &str = "https://erisscans.com";

struct ErisScans;

impl Impl for ErisScans {
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

register_source!(Keyoapp<ErisScans>, Home, DeepLinkHandler);
