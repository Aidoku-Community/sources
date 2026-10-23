#![no_std]
use aidoku::{Source, prelude::*};
use heancms::{HeanCms, Impl, Params};

const BASE_URL: &str = "https://omegascans.org";
const API_URL: &str = "https://api.omegascans.org";

struct OmegaScans;

impl Impl for OmegaScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
			latest_sort_ascending: false,
		}
	}
}

register_source!(HeanCms<OmegaScans>, DynamicFilters, DeepLinkHandler);
