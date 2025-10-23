#![no_std]
use aidoku::{Source, prelude::*};
use madtheme::{Impl, MadTheme, Params};

const BASE_URL: &str = "https://novelbuddy.com";

struct NovelBuddy;

impl Impl for NovelBuddy {
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

register_source!(MadTheme<NovelBuddy>, ImageRequestProvider, DeepLinkHandler);
