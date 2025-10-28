#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://sushiscan.net";

struct Sushiscan;

impl Impl for Sushiscan {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			manga_url_directory: "/catalogue".into(),
			date_format: "d MMMM yyyy".into(),
			date_locale: "fr_FR".into(),
			..Default::default()
		}
	}
}

register_source!(
	MangaThemesia<Sushiscan>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
