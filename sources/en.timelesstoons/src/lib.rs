#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{Impl, Keyoapp, Params};

const BASE_URL: &str = "https://timelesstoons.org";

struct TimelessToons;

impl Impl for TimelessToons {
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

register_source!(Keyoapp<TimelessToons>, Home, DeepLinkHandler);
