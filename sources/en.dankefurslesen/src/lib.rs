#![no_std]

use aidoku::{ContentRating, Source, Viewer, imports::net::{TimeUnit, set_rate_limit}, prelude::*};
use guya::{Guya, Impl, Params, SeriesDetail};

struct DankeMoe;

impl Impl for DankeMoe {
    fn new() -> Self {
        set_rate_limit(2, 2, TimeUnit::Seconds);
        Self
    }

    fn params(&self) -> Params {
        Params {
            base_url: "https://danke.moe",
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

register_source!(Guya<DankeMoe>, ListingProvider, DeepLinkHandler, ImageRequestProvider);
