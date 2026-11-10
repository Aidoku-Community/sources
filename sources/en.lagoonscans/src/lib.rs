#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://lagoonscans.com";

struct LagoonScans;

impl Impl for LagoonScans {
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
	MangaThemesia<LagoonScans>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
