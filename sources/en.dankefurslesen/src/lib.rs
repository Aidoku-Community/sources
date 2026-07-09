#![no_std]
use aidoku::{ContentRating, Source, Viewer, prelude::*};
use guya::{Guya, Impl, Params, SeriesDetail};

const BASE_URL: &str = "https://danke.moe";

struct DankeMoe;

impl Impl for DankeMoe {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL,
			viewer: Viewer::RightToLeft,
		}
	}

	fn content_rating_for(&self, det: &SeriesDetail) -> ContentRating {
		if det.adult {
			ContentRating::NSFW
		} else {
			ContentRating::Safe
		}
	}
}

register_source!(
	Guya<DankeMoe>,
	ListingProvider,
	DeepLinkHandler,
	ImageRequestProvider
);
