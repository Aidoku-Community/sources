#![no_std]
use aidoku::{prelude::*, DeepLinkHandler, Home, ImageRequestProvider, Source};
use mangabox::{Impl, MangaBox, Params};

const BASE_URL: &str = "https://www.mangakakalot.gg";

struct MangaKakalot;

impl Impl for MangaKakalot {
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
	MangaBox<MangaKakalot>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
