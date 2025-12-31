#![no_std]
use aidoku::{prelude::*, Source};
use iken::{Iken, Impl, Params};

const BASE_URL: &str = "https://eternalmangas.org";
const API_URL: &str = "https://api.eternalmangas.org";

struct MagusManga;

impl Impl for MagusManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: Some(API_URL.into()),
			..Default::default()
		}
	}
}

register_source!(Iken<MagusManga>, Home, DeepLinkHandler);
