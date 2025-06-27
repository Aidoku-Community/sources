#![no_std]
use aidoku::{prelude::*, Source};
use mangabox::{Impl, MangaBox, Params};

const BASE_URL: &str = "https://www.nelomanga.net";

struct MangaNelo;

impl Impl for MangaNelo {
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
	MangaBox<MangaNelo>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
