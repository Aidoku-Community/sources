#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{ImplV2, KeyoappV2, Params};

const BASE_URL: &str = "https://kaizenscan.com";

struct KaizenScan;

impl ImplV2 for KaizenScan {
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

register_source!(KeyoappV2<KaizenScan>, Home, DeepLinkHandler);
