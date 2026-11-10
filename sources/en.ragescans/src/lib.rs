#![no_std]
use aidoku::{Source, Viewer, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://ragescans.com";

struct RageScans;

impl Impl for RageScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			viewer_override: Some(Viewer::Webtoon),
			series_author_selector: ".tsinfo .imptdt:contains(Author) i".into(),
			series_artist_selector: ".tsinfo .imptdt:contains(Artist) i".into(),
			..Default::default()
		}
	}
}

register_source!(
	MangaThemesia<RageScans>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
