#![no_std]
use aidoku::{Source, prelude::*};
use keyoapp::{Impl, Keyoapp, Params};

const BASE_URL: &str = "https://genztoons.org";

struct GenzToon;

impl Impl for GenzToon {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			// The site's Pinned Series lists the same series as Latest Updates
			hidden_rows: &["Pinned Series"],
		}
	}
}

register_source!(Keyoapp<GenzToon>, Home, DeepLinkHandler);
