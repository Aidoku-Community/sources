#![no_std]

use aidoku::{ContentRating, Source, Viewer, imports::net::{TimeUnit, set_rate_limit}, prelude::*};
use guya::{Guya, Impl, Params};

struct DankeMoe;

impl Impl for DankeMoe {
    fn new() -> Self {
        set_rate_limit(2, 2, TimeUnit::Seconds);
        Self
    }

    fn params(&self) -> Params {
        Params {
            base_url: "https://danke.moe",
            content_rating: ContentRating::Suggestive,
            viewer: Viewer::RightToLeft,
        }
    }
}

register_source!(Guya<DankeMoe>, ListingProvider, Home, DeepLinkHandler, ImageRequestProvider);
