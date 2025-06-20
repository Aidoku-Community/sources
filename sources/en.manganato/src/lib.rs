#![no_std]
use aidoku::{prelude::*, DeepLinkHandler, Home, ImageRequestProvider, ListingProvider, Source};
use mangabox::{Impl, MangaBox, Params};

const BASE_URL: &str = "https://www.manganato.gg";

struct MangaNato;

impl Impl for MangaNato {
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
	MangaBox<MangaNato>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
