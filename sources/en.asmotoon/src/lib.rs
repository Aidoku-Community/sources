#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{ImplV2, KeyoappV2, Params};

const BASE_URL: &str = "https://asmotoon.com";

struct AsmodeusScans;

impl ImplV2 for AsmodeusScans {
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

register_source!(KeyoappV2<AsmodeusScans>, Home, DeepLinkHandler);
