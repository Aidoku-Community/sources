#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://manhwax.top";

struct Manhwax;

impl Impl for Manhwax {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			mark_all_nsfw: true,
			..Default::default()
		}
	}
}

register_source!(
	MangaThemesia<Manhwax>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
