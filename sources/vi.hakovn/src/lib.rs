#![no_std]
use aidoku::{
	alloc::{borrow::ToOwned, string::ToString, *},
	helpers::uri::QueryParameters,
	imports::{
		defaults::defaults_get,
		html::{Element, Html},
	},
	prelude::*,
	Chapter, FilterValue, Manga, Page, PageContent, Result, Source, Viewer,
};
use wpcomics::{helpers::extract_f32_from_string, Impl, Params, WpComics};

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604";

fn remove_node(node: Element, content_html: &mut String) {
	if let Some(node_html) = node.outer_html() {
		*content_html = content_html.replace(&node_html, "");
	}
}

struct Hako;

impl Impl for Hako {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		let manga_details_cover_transformer = |style: String| {
			// style="background-image: url('https://...');"
			let start = style.find("url('").map(|i| i + 5).unwrap_or(0);
			let end = style[start..]
				.find("')")
				.map(|i| start + i)
				.unwrap_or(style.len());
			style[start..end].to_string()
		};
		Params {
			base_url: defaults_get::<String>("url").unwrap_or_default().into(),
			viewer: Viewer::RightToLeft,

			next_page: ".next:not(.disabled)",
			manga_cell: ".row .thumb-item-flow",
			manga_cell_title: ".series-title a",
			manga_cell_url: ".series-title a",
			manga_cell_image: ".content.img-in-ratio",
			manga_cell_image_attr: "abs:data-bg",
			manga_parse_id: |url| {
				url.split_once("//")
					.map(|(_, rest)| rest)
					.unwrap_or_default()
					.split_once('/')
					.map(|(_, path)| path)
					.unwrap_or_default()
					.trim_start_matches('/')
					.to_string()
			},
			chapter_parse_id: |url| {
				url.trim_start_matches('/')
					.split("/")
					.last()
					.unwrap_or_default()
					.to_string()
			},

			manga_details_title: ".series-name > a",
			manga_details_cover: ".series-cover .img-in-ratio",
			manga_details_cover_attr: "style",
			manga_details_cover_transformer,
			manga_details_authors: ".info-name:contains(Tác giả) + span",
			manga_details_description: ".summary-wrapper",
			manga_details_tags: "a.series-gerne-item",
			manga_details_tags_splitter: "",
			manga_details_status: ".info-name:contains(Tình trạng) + span",

			user_agent: Some(USER_AGENT),

			get_search_url: |params, q, page, filters| {
				let mut excluded_tags: Vec<String> = Vec::new();
				let mut included_tags: Vec<String> = Vec::new();
				let mut query = QueryParameters::new();
				query.push("query", Some(&q.to_owned().unwrap_or_default()));
				query.push("keywords", Some(&q.to_owned().unwrap_or_default()));
				query.push("title", Some(&q.unwrap_or_default()));
				query.push("page", Some(&page.to_string()));

				if filters.is_empty() {
					return Ok(format!("{}/tim-kiem?{query}", params.base_url,));
				}

				for filter in filters {
					match filter {
						FilterValue::Text { id, value, .. } => {
							query.push(&id, core::prelude::v1::Some(&value));
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
					"{}/tim-kiem-nang-cao/?selectgenres={}&rejectgenres={}&{}",
					params.base_url,
					included_tags.join(","),
					excluded_tags.join(","),
					query
				))
			},

			home_manga_link: ".series-title a",
			home_chapter_link: ".chapter-title a",

			home_sliders_selector: ".slider",
			home_sliders_title_selector: "h2",
			home_sliders_item_selector: ".popular-thumb-item",

			home_grids_selector: ".index-section",
			home_grids_title_selector: ".section-title",
			home_grids_item_selector: ".thumb-item-flow",

			home_manga_cover_selector: ".content.img-in-ratio",
			home_manga_cover_slider_attr: Some("style"),
			home_manga_cover_slider_transformer: manga_details_cover_transformer,
			home_manga_cover_attr: "abs:data-bg",
			time_formats: Some(["%d/%m/%Y", "%m-%d-%Y", "%Y-%d-%m"].to_vec()),

			..Default::default()
		}
	}

	fn get_chapter_list(
		&self,
		cache: &mut wpcomics::Cache,
		params: &Params,
		url: String,
	) -> Result<Vec<Chapter>> {
		let html = self.cache_manga_page(cache, params, url.as_str())?;

		let html = Html::parse_with_url(html, url)?;
		let title_untrimmed = (params.manga_details_title_transformer)(
			html.select(params.manga_details_title)
				.and_then(|v| v.text())
				.unwrap_or_default(),
		);
		let title = title_untrimmed.trim();

		let Some(volumes_iter) = html.select(".volume-list") else {
			return Ok(vec![]);
		};

		let mut chapters = volumes_iter
			.filter_map(|volume_node| {
				let Some(chapters_iter) = volume_node.select(".list-chapters > li") else {
					return None;
				};

				let volume_title = volume_node
					.select_first(".sect-title")
					.and_then(|v| v.text())
					.unwrap_or_default();
				let volume_number = if volume_title.to_lowercase().contains("one shot") {
					-1.0
				} else {
					extract_f32_from_string(&title, &volume_title)
						.first()
						.map(|v| *v)
						.unwrap_or(-1.0)
				};
				let volume_title =
					if let Some((_, rest)) = volume_title.split_once(&volume_number.to_string()) {
						rest.trim_start_matches([':', '-', ' ']).trim().to_string()
					} else {
						volume_title.replace(title, "").trim().to_string()
					};
				let volume_thumbnail =
					volume_node
						.select_first(".content.img-in-ratio")
						.and_then(|node| {
							let style = node.attr("style")?;
							let url = (params.home_manga_cover_slider_transformer)(style);
							Some(url)
						});

				Some(
					chapters_iter
						.filter_map(|chapter_node| {
							let anchor_node = chapter_node.select_first("a")?;

							let chapter_url = anchor_node.attr("abs:href")?;

							let chapter_id = (params.chapter_parse_id)(chapter_url.to_owned());
							let chapter_title = anchor_node.text().unwrap_or_default();
							let chapter_title = chapter_title.trim();
							let chapter_number = extract_f32_from_string(&title, &chapter_title)
								.first()
								.map(|v| *v)
								.unwrap_or(-1.0);
							let chapter_title = if let Some((_, rest)) =
								chapter_title.split_once(&chapter_number.to_string())
							{
								rest.trim_start_matches([':', '-', ' ']).trim().to_string()
							} else {
								chapter_title.replace(title, "").trim().to_string()
							};

							let date_updated = (params.time_converter)(
								params,
								&chapter_node
									.select(".chapter-time")?
									.text()
									.unwrap_or_default(),
							);

							let chapter = Chapter {
								key: chapter_id,
								title: Some(
									format!(
										"{}{}{}",
										chapter_title,
										if volume_title.is_empty() { "" } else { " - " },
										volume_title
									)
									.to_string(),
								),
								volume_number: if volume_number < 0.0 {
									None
								} else {
									Some(volume_number)
								},
								chapter_number: if chapter_number < 0.0 {
									None
								} else {
									Some(chapter_number)
								},
								date_uploaded: Some(date_updated),
								url: Some(chapter_url),
								thumbnail: volume_thumbnail.to_owned(),
								..Default::default()
							};

							Some(chapter)
						})
						.collect::<Vec<_>>(),
				)
			})
			.flatten()
			.collect::<Vec<_>>();
		chapters.reverse();

		Ok(chapters)
	}

	fn get_page_list(
		&self,
		cache: &mut wpcomics::Cache,
		params: &Params,
		manga: Manga,
		chapter: Chapter,
	) -> Result<Vec<Page>> {
		let mut pages: Vec<Page> = Vec::new();

		let url = (params.page_list_page)(params, &manga, &chapter);
		let html = self.create_request(cache, params, &url, None)?.html()?;

		let Some(content) = html.select_first("#chapter-content") else {
			bail!("Failed to get chapter content");
		};
		let Some(mut content_html) = content.html() else {
			bail!("Failed to get chapter content HTML");
		};

		// modify html
		if let Some(list) =
			content.select(".d-none, script, #chapter-content > a[target='__blank']")
		{
			for node in list {
				remove_node(node, &mut content_html);
			}
		}
		if let Some(list) = content.select("[id^=\"note\"]") {
			for node in list {
				let none_print_node = node.select(".none-print.inline");
				if let Some(none_print_node) = none_print_node {
					for node in none_print_node {
						remove_node(node, &mut content_html);
					}
				}

				let note_content_node = node.select_first(".note-content").and_then(|v| v.parent());
				if let Some(note_content_node) = note_content_node {
					remove_node(note_content_node, &mut content_html);
				}
			}
		}

		if let Some(styles_node) = content.select("[style]") {
			for style_node in styles_node {
				if let Some(style) = style_node.attr("style") {
					let has_display_none = style.contains("display:")
						&& style[style.find("display:").unwrap_or_default()..].contains("none");
					if has_display_none {
						remove_node(style_node, &mut content_html);
					}
				}
			}
		}

		// edit notes
		if let Some(notes) = content.select("[id^=\"note\"]") {
			let ids = notes
				.into_iter()
				.filter_map(|node| node.attr("id"))
				.collect::<Vec<_>>();

			// Replace occurrences like [note123] with an anchor only if the id exists
			let original = content_html.clone();
			content_html = String::new();
			let mut last_idx: usize = 0;

			while let Some(rel_start) = original[last_idx..].find('[') {
				let start = last_idx + rel_start;
				// find closing bracket after start
				if let Some(rel_end) = original[start + 1..].find(']') {
					let end = start + 1 + rel_end; // index of ']'
									// append text before '['
					content_html.push_str(&original[last_idx..start]);
					let inner = &original[start + 1..end];
					let is_note = inner.len() > 4
						&& inner.starts_with("note")
						&& inner[4..].chars().all(|c| c.is_ascii_digit());
					if is_note && ids.iter().any(|id| id == inner) {
						content_html.push_str(&format!(
							"<a id=\"anchor-{id}\" href=\"#{id}\" class=\"note-link\">**</a>",
							id = inner
						));
					} else {
						// not a matching note id — keep original including brackets
						content_html.push_str(&original[start..=end]);
					}
					last_idx = end + 1;
					continue;
				}
				// no closing bracket found; stop searching
				break;
			}
			// append remaining tail
			content_html.push_str(&original[last_idx..]);
		}

		// remove comments
		while let Some(start) = content_html.find("<!--") {
			if let Some(end) = content_html[start..].find("-->") {
				let end_pos = start + end + 3;
				content_html.drain(start..end_pos);
			} else {
				break;
			}
		}

		// end modify html

		let description = html.select_first("h6.title-item").and_then(|v| v.text());

		pages.push(Page {
			content: PageContent::Text(format!("<!--html-->{content_html}")),
			has_description: description.is_some(),
			description,
			..Default::default()
		});

		Ok(pages)
	}
}

register_source!(WpComics<Hako>, ImageRequestProvider, DeepLinkHandler, Home);
