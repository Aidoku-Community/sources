#![no_std]
use aidoku::{prelude::*, Source};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://www.mangaread.org";

struct MangaReadOrg;

impl Impl for MangaReadOrg {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			datetime_format: "dd.MM.yyy".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<MangaReadOrg>,
	DeepLinkHandler,
	Home,
	MigrationHandler
);
