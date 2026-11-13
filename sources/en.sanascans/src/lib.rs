#![no_std]
use aidoku::{Source, prelude::*};
use iken::{Iken, Impl, Params};

const BASE_URL: &str = "https://sanascans.com";
const API_URL: &str = "https://api.sanascans.com";

struct SanaScans;

impl Impl for SanaScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
			page_size: 30,
			sort_pages_by_filename: true,
			..Default::default()
		}
	}
}

register_source!(Iken<SanaScans>, Home, DeepLinkHandler, ImageRequestProvider);
