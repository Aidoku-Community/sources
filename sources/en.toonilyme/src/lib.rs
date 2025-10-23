#![no_std]
use aidoku::{ContentRating, Source, Viewer, prelude::*};
use madtheme::{Impl, MadTheme, Params};

const BASE_URL: &str = "https://toonily.me";

struct ToonilyMe;

impl Impl for ToonilyMe {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_slug_search: true,
			default_rating: ContentRating::NSFW,
			default_viewer: Viewer::Webtoon,
			..Default::default()
		}
	}
}

register_source!(MadTheme<ToonilyMe>, ImageRequestProvider, DeepLinkHandler);
