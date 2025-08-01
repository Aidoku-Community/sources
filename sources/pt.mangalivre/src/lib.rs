#![no_std]
use aidoku::{prelude::*, Source};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://mangalivre.tv";

struct ManagLivre;

impl Impl for ManagLivre {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_new_chapter_endpoint: true,
			datetime_format: "MMMM dd, yyyy".into(),
			datetime_locale: "pt_BR".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<ManagLivre>,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
