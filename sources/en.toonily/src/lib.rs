#![no_std]
use aidoku::{imports::net::Request, prelude::*, Result, Source, Viewer};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://toonily.com";

struct Toonily;

impl Impl for Toonily {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			source_path: "serie".into(),
			default_viewer: Viewer::Webtoon,
			datetime_format: "MMM d, yy".into(),
			filter_non_manga_items: false,
			use_new_chapter_endpoint: true,
			use_load_more_request: madara::LoadMoreStrategy::Always,
			search_manga_selector: "div.page-item-detail.manga".into(),
			..Default::default()
		}
	}

	fn modify_request(&self, _params: &Params, request: Request) -> Result<Request> {
		Ok(request.header("Cookie", "toonily-mature=1"))
	}
}

register_source!(
	Madara<Toonily>,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);
