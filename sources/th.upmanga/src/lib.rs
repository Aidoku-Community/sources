#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://www.up-manga.com";

struct UpManga;

impl Impl for UpManga {
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
	MangaThemesia<UpManga>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
