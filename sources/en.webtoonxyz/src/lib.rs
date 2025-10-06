#![no_std]
use aidoku::{prelude::*, Source, Viewer};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://www.webtoon.xyz";

struct WebtoonXYZ;

impl Impl for WebtoonXYZ {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			source_path: "read".into(),
			default_viewer: Viewer::Webtoon,
			datetime_format: "dd MMMM yyyy".into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<WebtoonXYZ>,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);
