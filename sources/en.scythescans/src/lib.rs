#![no_std]
use aidoku::{
	Source,
	imports::net::{TimeUnit, set_rate_limit},
	prelude::*,
};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://scythescans.com";

struct ScytheScans;

impl Impl for ScytheScans {
	fn new() -> Self {
		set_rate_limit(3, 1, TimeUnit::Seconds);
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
	MangaThemesia<ScytheScans>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
