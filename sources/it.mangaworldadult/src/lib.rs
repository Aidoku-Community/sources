#![no_std]
use aidoku::{Source, prelude::*};
use mangaworld_template::{Impl, MangaWorldTemplate, Params};

const BASE_URL: &str = "https://www.mangaworldadult.net";

struct MangaWorldAdult;

impl Impl for MangaWorldAdult {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(MangaWorldTemplate<MangaWorldAdult>, DeepLinkHandler);
