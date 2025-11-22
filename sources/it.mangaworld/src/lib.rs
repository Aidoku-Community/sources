#![no_std]
use aidoku::{Source, prelude::*};
use mangaworld_template::{Impl, MangaWorldTemplate, Params};

const BASE_URL: &str = "https://www.mangaworld.mx";

struct MangaWorld;

impl Impl for MangaWorld {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
		}
	}
}

register_source!(MangaWorldTemplate<MangaWorld>, DeepLinkHandler);
