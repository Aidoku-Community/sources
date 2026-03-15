#![no_std]
use aidoku::{Result, Source, alloc::string::String, imports::net::Request, prelude::*};
use mangathemesia::{Impl, MangaThemesia, Params};

const BASE_URL: &str = "https://www.silentquill.net";

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

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url.replace("https:///", "https://"))?
			.header("Accept", "image/avif,image/webp,image/png,image/jpeg,*/*")
			.header("Referer", &format!("{}/", params.base_url)))
	}
}

register_source!(
	MangaThemesia<Armageddon>,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
