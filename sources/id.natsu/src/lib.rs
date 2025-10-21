#![no_std]
use aidoku::{Source, prelude::*};
use tukutema::{Impl, Params, Tukutema};

const BASE_URL: &str = "https://natsu.tv";

struct Natsu;

impl Impl for Natsu {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(Tukutema<Natsu>, Home, ListingProvider, DeepLinkHandler);
