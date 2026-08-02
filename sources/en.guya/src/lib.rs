#![no_std]
use aidoku::{Source, Viewer, prelude::*};
use guya::{Guya, Impl, Params};

const BASE_URL: &str = "https://guya.cubari.moe";

struct GuyaMoe;

impl Impl for GuyaMoe {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL,
			viewer: Viewer::RightToLeft,
		}
	}
}

register_source!(Guya<GuyaMoe>, DeepLinkHandler, ImageRequestProvider);
