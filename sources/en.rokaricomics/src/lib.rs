#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://rokaricomics.com";

struct RokariComics;

impl Impl for RokariComics {
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
	MangaThemesia<RokariComics>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
