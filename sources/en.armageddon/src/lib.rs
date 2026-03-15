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
			series_title_selector: "h1.kdt8-left-title".into(),
			series_cover_selector: ".kdt8-cover img".into(),
			series_description_selector: ".kdt8-synopsis".into(),
			series_genre_selector: ".kdt8-genres a.kdt8-genre-tag".into(),
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
