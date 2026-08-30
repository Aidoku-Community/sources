#![no_std]
use aidoku::{prelude::*, Source};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://3asq.online";

struct Manga3asq;

impl Impl for Manga3asq {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_new_chapter_endpoint: true,
			datetime_format: "d MMM\u{060c} yyy".into(),
			datetime_locale: "ar".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<Manga3asq>,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);
