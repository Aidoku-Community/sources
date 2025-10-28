#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://mangasusuku.com";

struct Mangasusu;

impl Impl for Mangasusu {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			manga_url_directory: "/komik".into(),
			mark_all_nsfw: true,
			..Default::default()
		}
	}
}

register_source!(
	MangaThemesia<Mangasusu>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
