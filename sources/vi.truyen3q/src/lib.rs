#![no_std]
use aidoku::{
	FilterValue, Source, Viewer,
	alloc::{string::ToString, *},
	helpers::uri::QueryParameters,
	prelude::*,
};
use wpcomics::{Impl, Params, WpComics};

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604";
const BASE_URL: &str = "https://truyentranh3qr.com";

struct Truyen3Q;

impl Impl for Truyen3Q {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: String::from(BASE_URL),
			viewer: Viewer::RightToLeft,

			next_page: ".page_redirect > a:nth-last-child(2) > p:not(.active)",
			manga_cell: "ul.grid li",
			manga_cell_title: ".book_info .qtip a",
			manga_cell_url: ".book_info .qtip a",
			manga_cell_image: ".book_avatar img",
			manga_cell_image_attr: "abs:src",
			manga_parse_id: |url| String::from(url.split("/").last().unwrap_or_default()),

			manga_details_title: "div.book_other h1[itemprop=name]",
			manga_details_cover: "div.book_avatar img",
			manga_details_cover_attr: "abs:src",
			manga_details_description: "div.story-detail-info.detail-content",
			manga_details_tags: "ul.list01 > li",
			manga_details_tags_splitter: "",
			manga_details_status: "li.status.row p.col-xs-9",
			manga_details_chapters: "div.works-chapter-item",

			chapter_skip_first: false,
			chapter_anchor_selector: "div.name-chap a",
			chapter_date_selector: "div.time-chap",
			chapter_parse_id: |url| String::from(url.split("/").last().unwrap_or_default()),

			page_url_transformer: |url| url,
			user_agent: Some(USER_AGENT),

			get_search_url: |params, q, page, filters| {
				let mut excluded_tags: Vec<String> = Vec::new();
				let mut included_tags: Vec<String> = Vec::new();
				let mut query = QueryParameters::new();
				query.push("keyword", Some(&q.unwrap_or_default()));
				query.push("page", Some(&page.to_string()));
				query.push("post_type", Some("wp-manga"));

				for filter in filters {
					match filter {
						FilterValue::MultiSelect {
							included, excluded, ..
						} => {
							for tag in included {
								included_tags.push(tag);
							}
							for tag in excluded {
								excluded_tags.push(tag);
							}
						}
						FilterValue::Select { id, value } => {
							query.push(&id, Some(&value));
						}
						FilterValue::Sort { id, index, .. } => {
							query.push(&id, Some(&index.to_string()));
						}
						_ => {}
					}
				}

				Ok(format!(
					"{}/tim-kiem-nang-cao/?categories={}&nocategories={}&{}",
					params.base_url,
					included_tags.join(","),
					excluded_tags.join(","),
					query
				))
			},

			home_manga_link: "h3 > a",
			home_chapter_link: ".last_chapter > a",
			home_date_uploaded: ".time-ago",
			home_date_uploaded_attr: "text",

			home_sliders_selector: ".homepage_suggest",
			home_sliders_title_selector: "h2",
			home_sliders_item_selector: ".item",

			home_grids_selector: "#main_homepage",
			home_grids_title_selector: "h1",
			home_grids_item_selector: "ul > li",

			home_manga_cover_attr: "abs:src",
			time_formats: Some(["%d/%m/%Y", "%m-%d-%Y", "%Y-%d-%m"].to_vec()),

			..Default::default()
		}
	}
}

register_source!(
	WpComics<Truyen3Q>,
	ImageRequestProvider,
	DeepLinkHandler,
	Home
);
