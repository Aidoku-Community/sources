#![no_std]
use aidoku::{Source, Viewer, prelude::*};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://grabber.zone";

struct GrabberZone;

impl Impl for GrabberZone {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			source_path: "comics".into(),
			default_viewer: Viewer::default(),
			use_load_more_request: madara::LoadMoreStrategy::Always,
			chapter_title_selector: "a:nth-of-type(2)".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<GrabberZone>,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);
