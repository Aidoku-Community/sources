#![no_std]
use aidoku::{Source, prelude::*};
use iken::{Iken, Impl, Params};

const BASE_URL: &str = "https://vortexscans.org";
const API_URL: &str = "https://api.vortexscans.org";

struct VortexScans;

impl Impl for VortexScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
			banner_width: Some(1920),
			banner_height: Some(1080),
			..Default::default()
		}
	}
}

register_source!(
	Iken<VortexScans>,
	Home,
	DeepLinkHandler,
	ImageRequestProvider
);
