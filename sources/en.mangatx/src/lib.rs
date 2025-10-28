#![no_std]
use aidoku::{Source, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://mangatx.cc";

struct MangaTx;

impl Impl for MangaTx {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			manga_url_directory: "/manga-list".into(),
			date_format: "dd-MM-yyyy".into(),
			mark_all_nsfw: true,
			..Default::default()
		}
	}
}

register_source!(
	MangaThemesia<MangaTx>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
