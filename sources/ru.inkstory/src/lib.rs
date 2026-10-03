#![no_std]
use aidoku::{Source, prelude::*};
use otakuovh::{Impl, OtakuOvh, Params};

struct InkStory;

impl Impl for InkStory {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: "https://ink-api.inuko.me".into(),
			domain: "inkstory.net".into(),
			service_name: "inkstory".into(),
			key_decryption: "UySkp0BzPhwlvP2V".into(),
		}
	}
}

register_source!(
	OtakuOvh<InkStory>,
	ListingProvider,
	Home,
	DeepLinkHandler,
	PageImageProcessor,
	DynamicListings
);
