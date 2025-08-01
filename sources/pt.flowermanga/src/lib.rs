#![no_std]
use aidoku::{prelude::*, Source};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://flowermanga.net";

struct FlowerManga;

impl Impl for FlowerManga {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_new_chapter_endpoint: false,
			use_load_more_request: madara::LoadMoreStrategy::Never,
			datetime_format: "d 'de' MMMMM 'de' yyyy".into(),
			datetime_locale: "pt_BR".into(),
			// some pages label it with "Type" and others with "Tipo"
			details_type_selector: format!(
				"{}, div.post-content_item:contains(Tipo) div.summary-content",
				Params::default().details_type_selector
			)
			.into(),
			..Default::default()
		}
	}
}

register_source!(
	Madara<FlowerManga>,
	ImageRequestProvider,
	DeepLinkHandler,
	Home,
	MigrationHandler
);
