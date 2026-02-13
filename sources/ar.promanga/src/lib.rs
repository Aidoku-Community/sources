#![no_std]
use aidoku::{prelude::*, Source};
use iken::{Iken, Impl, Params};

const BASE_URL: &str = "https://prochan.net";

struct ProManga;

impl Impl for ProManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: Some(BASE_URL.into()),
			// post endpoint requires postSlug instead of postId
			use_slug_series_keys: true,
			// chapter isTimeLocked key not available on post endpoint
			fetch_full_chapter_list: true,
			..Default::default()
		}
	}
}

register_source!(Iken<ProManga>, Home, DeepLinkHandler);
