#![no_std]
use aidoku::{Source, prelude::*};
use mangak::{Impl, Params};

const BASE_URL: &str = "https://mangak.io";
const API_URL: &str = "https://api.mangak.io";

struct MangaK;

impl Impl for MangaK {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
			featured_title: "Most Views".into(),
			featured_path: "titles/search?source=ranking&sort=views&limit=10".into(),
			latest_title: "Recently Updated".into(),
		}
	}
}

register_source!(
	mangak::MangaK<MangaK>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
