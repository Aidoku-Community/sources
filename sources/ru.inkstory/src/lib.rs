#![no_std]
use aidoku::{
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, Listing,
	ListingProvider, Manga, MangaPageResult, Page, Result, Source,
	alloc::{String, Vec, borrow::Cow},
	prelude::*,
};
use otakuovh::{Impl, OtakuOvh, Params};

struct InkStory;

impl Impl for InkStory {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: Cow::Owned("https://api.inkstory.net".into()),
			domain: Cow::Owned("inkstory.net".into()),
			service_name: Cow::Owned("inkstory".into()),
			key_decryption: Cow::Owned("UySkp0BzPhwlvP2V".into()),
		}
	}
}

register_source!(OtakuOvh<InkStory>, ListingProvider, Home, DeepLinkHandler, PageImageProcessor, DynamicListings);
