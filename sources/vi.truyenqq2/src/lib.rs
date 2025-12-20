#![no_std]
use aidoku::{
	FilterValue, Result, Source, Viewer,
	alloc::{string::ToString, *},
	helpers::uri::QueryParameters,
	imports::defaults::defaults_get,
	prelude::*,
};
use wpcomics::{Impl, Params, WpComics, helpers::urlencode};

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604";
const BASE_URL: &str = "https://truyenqq.online";

fn get_visit_read_id() -> Result<String> {
	Ok(defaults_get::<String>("visitReadId")
		.map(|v| v.trim_end_matches('/').to_string())
		.unwrap_or_default())
}

struct TruyenQQ2;

impl Impl for TruyenQQ2 {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		let cookie = Some(format!(
			"visit-read={}",
			get_visit_read_id().unwrap_or_default()
		));

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
					.unwrap()
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
				let mut included_tags: Vec<String> = Vec::new();
				let mut query = QueryParameters::new();
				query.push("q", Some(&q.unwrap_or_default()));
				query.push("page", Some(&page.to_string()));
				query.push("post_type", Some("wp-manga"));

				if filters.is_empty() {
					return Ok(format!(
						"{}/tim-kiem{}{query}",
						params.base_url,
						if query.is_empty() { "" } else { "?" }
					));
				}

				for filter in filters {
					match filter {
						FilterValue::Text { value, .. } => {
							let title = urlencode(value);
							if !title.is_empty() {
								return Ok(format!(
									"{}/tim-kiem?q={title}&page={page}",
									params.base_url
								));
							}
						}
						FilterValue::MultiSelect { included, .. } => {
							for tag in included {
								included_tags.push(tag);
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
					"{}/tim-kiem-nang-cao?categories={}&{}",
					params.base_url,
					included_tags.join(","),
					query
				))
			},

			time_formats: Some(["%d/%m/%Y", "%m-%d-%Y", "%Y-%d-%m"].to_vec()),

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
