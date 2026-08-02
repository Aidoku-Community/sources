#![no_std]
use aidoku::{
	DeepLinkResult, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString},
	prelude::*,
};
use madara::{Impl, LoadMoreStrategy, Madara, Params};

const BASE_URL: &str = "https://tortugaceviri.com";

struct TortugaCeviri;

impl Impl for TortugaCeviri {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			use_load_more_request: LoadMoreStrategy::Never,
			default_viewer: Viewer::RightToLeft,
			datetime_format: "d MMMM yyyy".into(),
			datetime_locale: "tr_TR".into(),
			details_status_selector: "div.post-content_item:contains(Durumu) div.summary-content"
				.into(),
			details_type_selector: "div.post-content_item:contains(Tür) div.summary-content".into(),
			..Default::default()
		}
	}

	fn get_manga_viewer(&self, str: &str, default: Viewer) -> Viewer {
		let trimmed = str.trim();
		match trimmed.to_ascii_lowercase().as_str() {
			"manga" => Viewer::RightToLeft,
			"manhwa" | "manhua" | "webtoon" => Viewer::Webtoon,
			_ if trimmed == "Çizgi Roman" => Viewer::LeftToRight,
			_ => default,
		}
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(key) = url.strip_prefix(params.base_url.as_ref()).map(String::from) else {
			return Ok(None);
		};
		let key = key
			.split('#')
			.next()
			.unwrap_or(&key)
			.split('?')
			.next()
			.unwrap_or(&key)
			.to_string();
		let parts: Vec<&str> = key.trim_matches('/').split('/').collect();

		if parts.len() >= 3 && parts[0] == params.source_path.as_ref() {
			let manga_key = format!("/{}/{}/", parts[0], parts[1]);
			Ok(Some(DeepLinkResult::Chapter { manga_key, key }))
		} else if parts.len() >= 2 && parts[0] == params.source_path.as_ref() {
			Ok(Some(DeepLinkResult::Manga { key }))
		} else {
			Ok(None)
		}
	}
}

register_source!(
	Madara<TortugaCeviri>,
	Home,
	DeepLinkHandler,
	MigrationHandler,
	ImageRequestProvider
);
