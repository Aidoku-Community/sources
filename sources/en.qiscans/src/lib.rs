#![no_std]
use aidoku::{Source, prelude::*};
use ezmanhwa::{EzManhwa, Impl, Params};

const BASE_URL: &str = "https://qimanga.org";
const API_URL: &str = "https://api.qimanhwa.com/api/v1";

struct QiScans;

impl Impl for QiScans {
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

register_source!(EzManhwa<QiScans>, Home, DeepLinkHandler);
