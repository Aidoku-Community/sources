#![no_std]
use aidoku::{Source, alloc::String, imports::defaults::defaults_get, prelude::*};
use mangabox::{Impl, MangaBox, Params};

const BASE_URL: &str = "https://www.mangakakalot.gg";

struct MangaKakalot;

impl Impl for MangaKakalot {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		let base_url = defaults_get::<String>("url").unwrap_or_else(|| BASE_URL.into());
		Params {
			base_url: base_url.into(),
			..Default::default()
		}
	}
}

register_source!(
	MangaBox<MangaKakalot>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);

#[cfg(test)]
mod test;
