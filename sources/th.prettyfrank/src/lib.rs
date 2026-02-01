#![no_std]
use aidoku::{
	Chapter, Manga, Page, PageContent, Result, Source,
	alloc::{string::String, vec::Vec},
	imports::net::Request,
	prelude::*,
};
use mangathemesia::{Impl, MangaThemesia, Params, helpers};

const BASE_URL: &str = "https://www.pretty-frank.com";

struct PrettyFrank;

impl Impl for PrettyFrank {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			..Default::default()
		}
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}", params.base_url, chapter.key);
		let response = Request::get(&url)?.string()?;

		let slice = format!(
			"[{}]",
			helpers::extract_between(&response, "IMAGES = [", "]").unwrap_or_default()
		);
		Ok(serde_json::from_str::<Vec<String>>(&slice)
			.unwrap_or_default()
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(if url.starts_with('/') {
					format!("{BASE_URL}{url}")
				} else {
					url
				}),
				..Default::default()
			})
			.collect())
	}
}

register_source!(
	MangaThemesia<PrettyFrank>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
