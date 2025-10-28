#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://kanzenin.info";

struct Kanzenin;

impl Impl for Kanzenin {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			date_locale: "id".into(),
			..Default::default()
		}
	}
}

register_source!(
	MangaThemesia<Kanzenin>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
