#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{Impl, Keyoapp, Params};

const BASE_URL: &str = "https://grimscans.com";

struct GrimScans;

impl Impl for GrimScans {
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

register_source!(Keyoapp<GrimScans>, Home, DeepLinkHandler);
