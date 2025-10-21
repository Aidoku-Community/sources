#![no_std]
use aidoku::{Source, prelude::*};
use tukutema::{Impl, Params, Tukutema};

const BASE_URL: &str = "https://rawkuma.net";

struct Rawkuma;

impl Impl for Rawkuma {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(Tukutema<Rawkuma>, Home, ListingProvider, DeepLinkHandler);
