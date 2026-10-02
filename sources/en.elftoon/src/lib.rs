#![no_std]
use aidoku::{Source, prelude::*};
use vinetheme::{Impl, Params, VineTheme};

const BASE_URL: &str = "https://elftoon.net";

struct ElfToon;

impl Impl for ElfToon {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			..Default::default()
		}
	}
}

register_source!(
	VineTheme<ElfToon>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
