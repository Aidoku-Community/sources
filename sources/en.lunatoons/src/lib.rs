#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{ImplV2, KeyoappV2, Params};

const BASE_URL: &str = "https://lunatoons.net";

struct LunaToons;

impl ImplV2 for LunaToons {
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

register_source!(KeyoappV2<LunaToons>, Home, DeepLinkHandler);
