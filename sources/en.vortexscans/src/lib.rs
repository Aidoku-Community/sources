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
			api_url: Some(API_URL.into()),
			fetch_full_chapter_list: true,
			hide_home_lists: true,
			..Default::default()
		}
	}
}

register_source!(Iken<VortexScans>, Home, DeepLinkHandler);
