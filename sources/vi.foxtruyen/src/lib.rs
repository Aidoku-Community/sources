#![no_std]
use aidoku::{
	alloc::{string::ToString, *},
	helpers::uri::QueryParameters,
	prelude::*,
	FilterValue, Source, Viewer,
};
use wpcomics::{helper::urlencode, Impl, Params, WpComics};

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604";
const BASE_URL: &str = "https://foxtruyen.com";

struct FoxTruyen;

impl Impl for FoxTruyen {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			cookie: Some("type_book=1".to_string()),
			next_page: ".page_redirect > a:nth-last-child(2) > p:not(.active)",
			viewer: Viewer::RightToLeft,
			manga_cell: ".item_home",
			manga_cell_title: ".book_name",
			manga_cell_url: ".book_name",
			manga_cell_image: "img",
			manga_cell_image_attr: "data-src",

			manga_details_title: ".title_tale h1",
			manga_details_cover: ".thumbblock img",
			manga_details_chapters: ".item_chap",
			chapter_anchor_selector: "a",
			chapter_date_selector: "em",

			manga_parse_id: |url| {
				String::from(
					url.split("truyen-tranh/")
						.nth(1)
						.and_then(|s| s.split('/').next())
						.unwrap_or_default()
						.trim_end_matches(".html"),
				)
			},
			chapter_parse_id: |url| {
				String::from(
					url.trim_end_matches('/')
						.rsplit("-chap-")
						.next()
						.unwrap()
						.trim_end_matches(".html"),
				)
			},
			manga_viewer_page: ".content_detail_manga img",

			manga_details_authors: ".org",
			manga_details_description: "div.story-detail-info.detail-content",
			manga_details_tags: "ul.list01 > li",
			manga_details_tags_splitter: "",
			manga_details_status: "li.status.row p.col-xs-9",

			chapter_skip_first: false,
			page_url_transformer: |url| url,
			user_agent: Some(USER_AGENT),

			search_page: |page| format!("tim-kiem/trang-{}.html", page),
			manga_page: |params, manga| format!("{}/truyen-tranh/{}", params.base_url, manga.key),
			page_list_page: |params, manga, chapter| {
				format!(
					"{}/truyen-tranh/{}-chap-{}",
					params.base_url, manga.key, chapter.key
				)
			},

			get_search_url: |params, q, page, filters| {
				let mut excluded_tags: Vec<String> = Vec::new();
				let mut included_tags: Vec<String> = Vec::new();
				let mut query = QueryParameters::new();
				query.push("q", Some(&q.unwrap_or_default()));
				query.push("post_type", Some("wp-manga"));

				if filters.is_empty() {
					return Ok(format!(
						"{}/{}{}{query}",
						params.base_url,
						(params.search_page)(page),
						if query.is_empty() { "" } else { "?" }
					));
				}

				for filter in filters {
					match filter {
						FilterValue::Text { value, .. } => {
							let title = urlencode(value);
							if !title.is_empty() {
								return Ok(format!(
									"{}/tim-kiem/trang-{page}.html?q={title}",
									params.base_url
								));
							}
						}
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
					"{}/tim-kiem-nang-cao/trang-{}.html?category={}&notcategory={}&{}",
					params.base_url,
					page,
					included_tags.join(","),
					excluded_tags.join(","),
					query
				))
			},

			home_manga_link: ".book_name, .fs14",
			home_chapter_link: ".cl99",
			home_date_uploaded: ".time-ago, .timediff a",
			home_date_uploaded_attr: "text",

			home_sliders_selector: ".homepage_suggest",
			home_sliders_title_selector: "h2",
			home_sliders_item_selector: "li",

			home_grids_selector: "section > div > .col-md-6, .container > section:nth-child(1)",
			home_grids_title_selector: ".title_cate",
			home_grids_item_selector: ".item_home",

			home_manga_cover_attr: "abs:data-src",
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
