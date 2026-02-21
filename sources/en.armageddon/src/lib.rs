#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://www.silentquill.net";

struct Armageddon;

impl Impl for Armageddon {
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
	MangaThemesia<Armageddon>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
