#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://drakecomic.org";

struct DrakeScans;

impl Impl for DrakeScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			chapter_list_selector: "#chapterlist li:not(.locked)".into(),
			..Default::default()
		}
	}
}

register_source!(
	MangaThemesia<DrakeScans>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
