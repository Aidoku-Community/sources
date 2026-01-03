#![no_std]
use aidoku::{
	FilterValue, Source, Viewer,
	alloc::{string::ToString, *},
	helpers::uri::QueryParameters,
	imports::defaults::defaults_get,
	prelude::*,
};
use wpcomics::{Impl, Params, WpComics};

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604";
const BASE_URL: &str = "https://www.truyenqq.online";

fn get_visit_read_id() -> String {
	defaults_get::<String>("visitReadId")
		.map(|v| v.trim_end_matches('/').to_string())
		.unwrap_or_default()
}

struct TruyenQQ2;

impl Impl for TruyenQQ2 {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		let cookie = Some(format!("visit-read={}", get_visit_read_id()));

		Params {
			base_url: BASE_URL.into(),
			cookie,
			viewer: Viewer::RightToLeft,

			next_page: ".page_redirect > a:nth-last-child(2) > p:not(.active)",
			manga_cell: "ul.grid li",
			manga_cell_title: ".book_info .qtip a",
			manga_cell_url: ".book_info .qtip a",
			manga_cell_image: ".book_avatar img",
			manga_cell_image_attr: "abs:src",
			manga_cell_no_data: |node| {
				node.text()
					.map(|s| s.contains("Chưa có dữ liệu"))
					.unwrap_or(false)
			},
			manga_parse_id: |url| {
				url.split("truyen-tranh/")
					.nth(1)
					.and_then(|s| s.split('/').next())
					.unwrap_or_default()
					.into()
			},
			chapter_parse_id: |url| {
				url.rsplit_once("chapter/")
					.map(|(_, tail)| tail.trim_end_matches(".html"))
					.unwrap_or_default()
					.into()
			},

			manga_details_title: "div.book_other h1[itemprop=name]",
			manga_details_cover: "div.book_avatar img",
			manga_details_authors: "li.author.row p.col-xs-9",
			manga_details_description: "div.story-detail-info.detail-content",
			manga_details_tags: "ul.list01 > li",
			manga_details_tags_splitter: "",
			manga_details_status: "li.status.row p.col-xs-9",
			manga_details_chapters: "div.works-chapter-item",

			chapter_skip_first: false,
			chapter_anchor_selector: "div.name-chap a",
			chapter_date_selector: "div.time-chap",

			page_url_transformer: |url| url,
			user_agent: Some(USER_AGENT),

			manga_page: |params, manga| format!("{}/truyen-tranh/{}", params.base_url, manga.key),
			page_list_page: |params, manga, chapter| {
				format!(
					"{}/truyen-tranh/{}/chapter/{}",
					params.base_url, manga.key, chapter.key
				)
			},

			get_search_url: |params, q, page, filters| {
				let mut query = QueryParameters::new();
				query.push("q", q.as_deref());
				query.push("page", Some(&page.to_string()));
				query.push("post_type", Some("wp-manga"));

				if filters.is_empty() {
					return Ok(format!("{}/tim-kiem?{query}", params.base_url));
				}

				for filter in filters {
					match filter {
						FilterValue::Text { value, .. } => {
							let title = aidoku::helpers::uri::encode_uri_component(value);
							if !title.is_empty() {
								return Ok(format!(
									"{}/tim-kiem?q={title}&page={page}",
									params.base_url
								));
							}
						}
						FilterValue::MultiSelect { included, .. } => {
							query.push("categories", Some(&included.join(",")));
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

				Ok(format!("{}/tim-kiem-nang-cao?{query}", params.base_url))
			},

			time_formats: Some(vec!["%d/%m/%Y", "%m-%d-%Y", "%Y-%d-%m"]),

			..Default::default()
		}
	}
}

register_source!(
	WpComics<TruyenQQ2>,
	ImageRequestProvider,
	DeepLinkHandler,
	Home
);
