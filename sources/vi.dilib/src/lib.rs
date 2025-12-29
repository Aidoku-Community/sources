#![no_std]
use aidoku::{
	Chapter, FilterValue, Manga, Result, Source, Viewer,
	alloc::{borrow::ToOwned, string::ToString, *},
	helpers::uri::QueryParameters,
	imports::{html::Html, std::send_partial_result},
	prelude::*,
};
use wpcomics::{Cache, Impl, Params, WpComics, helpers::extract_f32_from_string};

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604";
const BASE_URL: &str = "https://dilib.vn";

struct DiLib;

impl Impl for DiLib {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			custom_headers: Some(vec![(
				"accept-language",
				// should accept maybe > 0.5
				"vi-VN,vi;q=0.9,en;q=0.8,ja;q=0.7",
			)]),
			viewer: Viewer::RightToLeft,

			next_page: ".end_link:not(.pagecurrent)",
			manga_cell: ".products > .type-product",
			manga_cell_title: "h3 a",
			manga_cell_url: "h3 a",
			manga_cell_image: "img",
			manga_cell_image_attr: "abs:src",
			manga_parse_id: |url| {
				url.trim_end_matches(".html")
					.split("/")
					.last()
					.unwrap_or_default()
					.to_string()
			},
			chapter_parse_id: |url| {
				url.rsplit_once("-chap-")
					.map(|(_, tail)| tail.trim_end_matches(".html"))
					.unwrap_or_default()
					.to_string()
			},

			manga_details_title: "#primary > div.section > div.col-md-7 > div > h1",
			manga_details_cover: "#primary img",
			manga_details_cover_attr: "abs:src",
			manga_details_authors: ".mt10.mb10:contains(Tác giả) > a",
			manga_details_description: "p[data-sourcepos]",
			manga_details_tags: "fieldset legend + a.button2",
			manga_details_tags_splitter: "",
			manga_details_status: ".mt10.mb10:contains(Tình trạng)",
			manga_details_status_transformer: |value| {
				value
					.split(":")
					.last()
					.unwrap_or_default()
					.trim()
					.trim_end_matches(".")
					.trim()
					.to_owned()
			},

			manga_details_chapters: "fieldset > .row > .col-md-3",
			chapter_anchor_selector: "a",

			manga_viewer_page: "#primary > img",

			user_agent: Some(USER_AGENT),

			get_search_url: |params, q, page, filters| {
				let mut query = QueryParameters::new();
				query.push("find", q.as_deref());
				query.push("page", Some(&page.to_string()));

				for filter in filters {
					match filter {
						FilterValue::Text { id, value, .. } => {
							query.push(&id, Some(&value));
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

				Ok(format!("{}/search.php?{}", params.base_url, query))
			},

			manga_page: |_, manga| format!("{}/{}.html", BASE_URL, manga.key).into(),
			page_list_page: |_, manga, chapter| {
				format!(
					"{}/truyen-tranh/{}-chap-{}.html",
					BASE_URL, manga.key, chapter.key
				)
			},

			home_manga_link: "a:nth-child(2)",
			home_chapter_link: "a",

			home_sliders_selector: "section[id^=\"demos\"]",
			home_sliders_title_selector: "h2",
			home_sliders_item_selector: ".owl-carousel > div",

			time_formats: Some(["%d/%m/%Y", "%m-%d-%Y", "%Y-%d-%m"].to_vec()),
			time_converter: |_, _| -1,

			..Default::default()
		}
	}

	fn get_manga_update(
		&self,
		cache: &mut Cache,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = (params.manga_page)(params, &manga);

		if needs_details {
			let new_manga = self.parse_manga_element(cache, params, url.clone())?;

			manga.copy_from(new_manga);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let chapters = self.get_chapter_list(cache, params, url.clone())?;

			if chapters.is_empty() {
				let html_data = self.cache_manga_page(cache, params, &url)?;
				let html = Html::parse_with_url(html_data, url)?;

				if let Some(url) = html
					.select_first("#primary > a[target=_blank].button1")
					.and_then(|node| node.attr("abs:href"))
				{
					let html_data = self.cache_manga_page(cache, params, &url)?;
					let html = Html::parse_with_url(html_data, url.clone())?;

					manga.chapters = html.select(".select > select.br5 > option").map(|options| {
						let mut chapters =
							options
								.into_iter()
								.map(|node| {
									let chapter_id = (params.chapter_parse_id)(
										node.attr("value").unwrap_or_default(),
									);
									let raw_chapter_title = node.text().unwrap_or_default();
									let numbers = extract_f32_from_string("", &raw_chapter_title);
									let (volume_number, chapter_number) = if numbers.len() > 1
										&& raw_chapter_title.to_ascii_lowercase().contains("vol")
									{
										(numbers[0], numbers[1])
									} else if !numbers.is_empty() {
										(-1.0, numbers[0])
									} else {
										(-1.0, -1.0)
									};
									let mut new_chapter_title = None;
									if chapter_number >= 0.0 {
										let splitter = format!(" {}", chapter_number);
										let splitter2 = format!("#{}", chapter_number);
										if raw_chapter_title.contains(&splitter) {
											let split = raw_chapter_title
												.splitn(2, &splitter)
												.collect::<Vec<&str>>();
											new_chapter_title = Some(
												String::from(split[1]).replacen([':', '-'], "", 1),
											);
										} else if raw_chapter_title.contains(&splitter2) {
											let split = raw_chapter_title
												.splitn(2, &splitter2)
												.collect::<Vec<&str>>();
											new_chapter_title = Some(
												String::from(split[1]).replacen([':', '-'], "", 1),
											);
										}
									}

									let chapter_title = new_chapter_title
										.and_then(|s| {
											let trimmed = s.trim();
											if trimmed.is_empty() {
												None
											} else {
												Some(trimmed.into())
											}
										})
										.or_else(|| Some(raw_chapter_title.trim().into()));

									Chapter {
										key: chapter_id.clone(),
										title: chapter_title,
										volume_number: if volume_number < 0.0 {
											None
										} else {
											Some(volume_number)
										},
										chapter_number: if chapter_number < 0.0
											&& volume_number >= 0.0
										{
											None
										} else {
											Some(chapter_number)
										},
										url: Some(format!(
											"{}-chap-{}.html",
											url.trim_end_matches(".html")
												.split("-chap-")
												.next()
												.unwrap_or_default(),
											chapter_id
										)),
										..Default::default()
									}
								})
								.collect::<Vec<_>>();

						chapters.reverse();

						chapters
					});
				}
			} else {
				manga.chapters = Some(chapters);
			}
		}

		Ok(manga)
	}
}

register_source!(WpComics<DiLib>, ImageRequestProvider, DeepLinkHandler, Home);
