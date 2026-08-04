#![no_std]
use aidoku::{Source, prelude::*};
use ezmanhwa::{EzManhwa, Impl, Params};

const BASE_URL: &str = "https://ezmanga.org";
const API_URL: &str = "https://vapi.ezmanga.org/api/v1";

struct EzManga;

impl Impl for EzManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
		}
	}
}

register_source!(EzManhwa<EzManga>, ListingProvider, Home, DeepLinkHandler);
