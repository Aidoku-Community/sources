#![no_std]
use aidoku::{Source, prelude::*};
use iken::{Iken, Impl, Params};

const BASE_URL: &str = "https://en-hijala.com";
const API_URL: &str = "https://api.en-hijala.com";

struct HijalaScans;

impl Impl for HijalaScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
			..Default::default()
		}
	}
}

register_source!(
	Iken<HijalaScans>,
	Home,
	DeepLinkHandler,
	ImageRequestProvider
);
