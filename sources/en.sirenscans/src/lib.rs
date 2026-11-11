#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{ImplV2, KeyoappV2, Params};

const BASE_URL: &str = "https://sirenscans.com";

struct SirenScans;

impl ImplV2 for SirenScans {
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

register_source!(KeyoappV2<SirenScans>, Home, DeepLinkHandler);
