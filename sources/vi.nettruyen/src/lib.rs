#![no_std]
use aidoku::{
	FilterValue, Source, Viewer,
	alloc::{string::ToString, *},
	helpers::uri::QueryParameters,
	prelude::*,
};
use wpcomics::{Impl, Params, WpComics};

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604";
const BASE_URL: &str = "https://nettruyenviet1.com";

struct FoxTruyen;

impl Impl for FoxTruyen {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: String::from(BASE_URL),

			next_page: "li.active + li > a[title*=\"kết quả\"]",
			viewer: Viewer::RightToLeft,

			manga_parse_id: |url| {
				String::from(
					url.split("truyen-tranh/")
						.nth(1)
						.and_then(|s| s.split('/').next())
						.unwrap_or_default(),
				)
			},
			chapter_parse_id: |url| {
				String::from(url.trim_end_matches('/').rsplit('/').next().unwrap())
			},

			user_agent: Some(USER_AGENT),
			manga_details_description: "div.detail-content > .shortened",

			manga_page: |params, manga| format!("{}/truyen-tranh/{}", params.base_url, manga.key),
			page_list_page: |params, manga, chapter| {
				format!(
					"{}/truyen-tranh/{}/{}",
					params.base_url, manga.key, chapter.key
				)
			},

			get_search_url: |params, q, page, filters| {
				let mut query = QueryParameters::new();
				query.push("keyword", Some(&q.unwrap_or_default()));
				query.push("post_type", Some("wp-manga"));
				query.push("page", Some(&page.to_string()));

				if filters.is_empty() {
					return Ok(format!(
						"{}/{}{query}",
						params.base_url,
						if query.is_empty() { "" } else { "?" }
					));
				}

				let mut tag = "".to_string();

				for filter in filters {
					match filter {
						FilterValue::Select { id, value } => {
							if id == "tag" {
								tag = value.clone();
							} else {
								query.push(&id, Some(&value));
							}
						}
						FilterValue::Sort { id, index, .. } => {
							query.push(&id, Some(&index.to_string()));
						}
						_ => {}
					}
				}

				Ok(format!(
					"{}/tim-truyen/{}{}{query}",
					params.base_url,
					tag,
					if query.is_empty() { "" } else { "?" }
				))
			},

			home_manga_link: "h3 > a",
			home_chapter_link: ".slide-caption > a, .chapter > a",
			home_date_uploaded: ".time",
			home_date_uploaded_attr: "text",

			home_sliders_selector: ".owl-carousel",
			home_sliders_title_selector: "h2",
			home_sliders_item_selector: ".item",

			home_grids_selector: ".items",
			home_grids_title_selector: ".page-title",
			home_grids_item_selector: ".item",

			home_manga_cover_attr: "abs:data-original",
			time_formats: Some(["%d/%m/%Y", "%m-%d-%Y", "%Y-%d-%m"].to_vec()),

			..Default::default()
		}
	}
}

register_source!(
	WpComics<FoxTruyen>,
	ImageRequestProvider,
	DeepLinkHandler,
	Home
);
