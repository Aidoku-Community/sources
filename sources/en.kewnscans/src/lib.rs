#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{Impl, Keyoapp, Params};

const BASE_URL: &str = "https://kewnscans.org";

struct KewnScans;

impl Impl for KewnScans {
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

register_source!(Keyoapp<KewnScans>, Home, DeepLinkHandler);
