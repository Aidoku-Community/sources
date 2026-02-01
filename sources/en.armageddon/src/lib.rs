#![no_std]
use aidoku::{
	AidokuError, Chapter, Manga, Page, PageContent, Result, Source,
	alloc::{string::String, vec::Vec},
	helpers::uri::internal_encode_uri,
	imports::net::Request,
	prelude::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://www.silentquill.net";

mod helpers;

struct Armageddon;

impl Impl for Armageddon {
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
		let html = Request::get(&url)?.html()?;

		let script_data = html
			.select_first("#kdt-secure-reader + script")
			.and_then(|el| el.data())
			.ok_or_else(|| error!("Script data not found"))?;

		let Some(base64_data) = helpers::extract_page_base64(&script_data) else {
			bail!("Page data not found");
		};

		let decoded_bytes = STANDARD
			.decode(base64_data)
			.map_err(|e| error!("Base64 decode error: {e}"))?;
		let decoded_str =
			core::str::from_utf8(&decoded_bytes).map_err(|e| error!("UTF-8 error: {e}"))?;
		let images: Vec<String> =
			serde_json::from_str(decoded_str).map_err(AidokuError::JsonParseError)?;

		let pages: Vec<Page> = images
			.into_iter()
			.map(|url| {
				let url = internal_encode_uri(url.as_bytes(), b";,/?:@&=+$-_.!~*'()#%");
				Page {
					content: PageContent::url(url),
					..Default::default()
				}
			})
			.collect();

		Ok(pages)
	}
}

register_source!(
	MangaThemesia<Armageddon>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
