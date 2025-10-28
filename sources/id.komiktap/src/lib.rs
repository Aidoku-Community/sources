#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://komiktap.info";

struct Komiktap;

impl Impl for Komiktap {
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
	MangaThemesia<Komiktap>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
