#![no_std]
use aidoku::{Source, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://firescans.xyz";

struct FireScans;

impl Impl for FireScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_new_chapter_endpoint: true,
			..Default::default()
		}
	}
}

register_source!(
	Madara<FireScans>,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);
