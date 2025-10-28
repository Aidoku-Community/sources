#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://01.komiku.asia";

struct KomikuCom;

impl Impl for KomikuCom {
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
	MangaThemesia<KomikuCom>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
