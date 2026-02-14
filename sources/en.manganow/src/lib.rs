#![no_std]
use aidoku::{alloc::String, prelude::*, DeepLinkResult, Result, Source};
use mangareader::{Impl, MangaReader, Params};

const BASE_URL: &str = "https://manganow.to";

struct MangaNow;

impl Impl for MangaNow {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			page_selector: ".container-reader-chapter > .iv-card:not([data-url$=manganow.jpg])"
				.into(),
			..Default::default()
		}
	}

	// same handler as mangabox
	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		const MANGA_PATH: &str = "manga/";
		if !path.starts_with(MANGA_PATH) {
			return Ok(None);
		}

		if let Some(idx) = path.rfind("/chapter-") {
			// ex: https://manganow.to/manga/i-was-reincarnated-as-an-evil-noble-in-a-game-and-became-unparalleled-with-my-overpowered-muscles/chapter-4
			let manga_key = &path[..idx];
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: path.into(),
			}))
		} else {
			// ex: https://manganow.to/manga/i-was-reincarnated-as-an-evil-noble-in-a-game-and-became-unparalleled-with-my-overpowered-muscles
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		}
	}
}

register_source!(
	MangaReader<MangaNow>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
