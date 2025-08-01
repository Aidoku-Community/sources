#![no_std]
use aidoku::{prelude::*, Source, Viewer};
use madara::{Impl, Madara, Params};

const BASE_URL: &str = "https://catharsisworld.dig-it.info";

struct CatharsisWorld;

impl Impl for CatharsisWorld {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			source_path: "serie".into(),
			use_new_chapter_endpoint: true,
			use_style_images: true,
			use_load_more_request: madara::LoadMoreStrategy::Always,
			default_viewer: Viewer::Webtoon,
			search_manga_selector: "button.group > div.grid".into(),
			search_manga_url_selector: "a".into(),
			search_manga_title_selector: "h3".into(),
			search_manga_cover_selector: "div[style].bg-cover".into(),
			details_title_selector: "div.wp-manga div.grid > h1".into(),
			details_cover_selector: "div.wp-manga > div.grid div.grid > div[style].bg-cover".into(),
			details_status_selector: "div.wp-manga div[alt=type]:eq(0) > span".into(),
			details_tag_selector: "div.wp-manga div[alt=type]:gt(0) > span".into(),
			details_description_selector: "div.wp-manga div#expand_content".into(),
			chapter_selector: "ul#list-chapters li > a".into(),
			chapter_title_selector: "div.grid > span".into(),
			chapter_date_selector: "div.grid > div".into(),
			chapter_thumbnail_selector: "div[style].bg-cover".into(),
			chapter_protector_password_prefix: "protectornonce='".into(),
			chapter_protector_data_prefix: "_data='".into(),
			datetime_locale: "es".into(),
			..Default::default()
		}
	}
}

register_source!(Madara<CatharsisWorld>, ListingProvider, DeepLinkHandler);
