use crate::helper::{extract_f32_from_string, find_first_f32, text_with_newlines};

use super::Params;
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, Filter, FilterValue, HomeComponent, HomeLayout, Manga,
	MangaPageResult, MangaWithChapter, MultiSelectFilter, Page, PageContent, PageContext, Result,
	Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::{
		html::{Element, Html},
		net::{HttpMethod, Request},
		std::send_partial_result,
	},
	prelude::*,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_cache_manga_id(&self, params: &mut Params) -> Option<String> {
		params.cache_manga_id.clone()
	}
	fn set_cache_manga_id(&self, params: &mut Params, value: Option<String>) {
		params.cache_manga_id = value;
	}

	fn get_cache_manga_value(&self, params: &mut Params) -> Option<Vec<u8>> {
		params.cache_manga_value.clone()
	}
	fn set_cache_manga_value(&self, params: &mut Params, value: Option<Vec<u8>>) {
		params.cache_manga_value = value;
	}

	fn cache_manga_page(&self, params: &mut Params, url: &str) -> Result<()> {
		let cached_id = self.get_cache_manga_id(params).clone();

		if cached_id == Some(url.to_string()) {
			return Ok(());
		}

		let req = self.create_request(params, url, None)?;

		self.set_cache_manga_value(params, Some(req.data()?));
		self.set_cache_manga_id(params, Some(url.to_string()));

		Ok(())
	}

	fn create_request(
		&self,
		params: &mut Params,
		url: &str,
		headers: Option<&[(&str, &str)]>,
	) -> Result<Request> {
		// 通常のリクエスト
		let mut req = Request::new(url, HttpMethod::Get)?;
		if let Some(cookie) = &params.cookie {
			req = req.header("Cookie", cookie);
		}
		if let Some(user_agent) = params.user_agent {
			req = req.header("User-Agent", user_agent);
		}
		if let Some(extra_headers) = headers {
			for (key, value) in extra_headers {
				req = req.header(key, value);
			}
		}
		Ok(self.modify_request(params, req)?)
	}

	fn category_parser(
		&self,
		params: &mut Params,
		categories: &Vec<String>,
	) -> (ContentRating, Viewer) {
		#[allow(clippy::needless_match)]
		let mut nsfw = params.nsfw;
		#[allow(clippy::needless_match)]
		let mut viewer = params.viewer;
		for category in categories {
			match category.to_ascii_uppercase().as_str() {
				"smut" | "mature" | "18+" | "adult" => nsfw = ContentRating::NSFW,
				"ecchi" | "16+" => {
					nsfw = match nsfw {
						ContentRating::NSFW => ContentRating::NSFW,
						_ => ContentRating::Suggestive,
					}
				}
				"webtoon" | "manhwa" | "manhua" => viewer = Viewer::Webtoon,
				_ => continue,
			}
		}
		(nsfw, viewer)
	}
	fn get_manga_list(
		&self,
		params: &mut Params,
		search_url: String,
		headers: Option<&[(&str, &str)]>,
	) -> Result<MangaPageResult> {
		let mut has_next_page = !params.next_page.is_empty();

		let html = self.create_request(params, &search_url, headers)?.html()?;

		let Some(elems) = html.select(params.manga_cell) else {
			return Ok(MangaPageResult {
				entries: vec![],
				has_next_page: false,
			});
		};
		let mut entries: Vec<Manga> = Vec::with_capacity(elems.size());
		for item_node in elems {
			let title = item_node
				.select(params.manga_cell_title)
				.and_then(|node| node.first())
				.and_then(|n| n.text());
			let url = item_node
				.select(params.manga_cell_url)
				.and_then(|node| node.first())
				.and_then(|n| n.attr("abs:href"))
				.unwrap_or("".to_string());

			let cover = if !params.manga_cell_image.is_empty() {
				item_node
					.select(params.manga_cell_image)
					.and_then(|v| v.first())
					.and_then(|n| n.attr(params.manga_cell_image_attr))
			} else {
				None
			};
			entries.push(Manga {
				key: (params.manga_parse_id)(url).to_string(),
				cover,
				title: (params.manga_details_title_transformer)(title.unwrap_or("".to_string()))
					.to_string(),
				..Default::default()
			});
		}
		if !params.next_page.is_empty() {
			has_next_page = html
				.select(params.next_page)
				.map(|v| v.size() > 0)
				.unwrap_or(false);
		}
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn parse_manga_element(&self, params: &mut Params, url: String) -> Result<Manga> {
		self.cache_manga_page(params, url.as_str())?;

		let html = self.get_cache_manga_value(params).unwrap();
		let details = Html::parse_with_url(html, url.clone())?;

		let title = details
			.select(params.manga_details_title)
			.and_then(|n| n.text());
		let cover = details
			.select(params.manga_details_cover)
			.and_then(|n| n.first())
			.and_then(|n| n.attr(params.manga_details_cover_attr));

		let authors = Some((params.manga_details_authors_transformer)(
			details
				.select(params.manga_details_authors)
				.map(|l| {
					l.map(|node| String::from(node.text().unwrap_or("".to_string()).trim()))
						.filter(|s| !s.is_empty())
						.collect()
				})
				.unwrap_or_default(),
		));
		let description = details
			.select(params.manga_details_description)
			.and_then(|l| l.first())
			.map(text_with_newlines);
		let mut tags = Vec::new();

		if !params.manga_details_tags.is_empty() {
			if params.manga_details_tags_splitter.is_empty() {
				tags = details
					.select(params.manga_details_tags)
					.map(|list| {
						list.map(|elem| elem.text().unwrap_or("".to_string()))
							.collect::<Vec<_>>()
					})
					.unwrap_or(vec![]);
			} else {
				let split = details.select(params.manga_details_tags).map(|l| {
					l.text()
						.unwrap_or_default()
						.split(params.manga_details_tags_splitter)
						.map(|s| s.trim().to_string())
						.filter(|s| !s.is_empty())
						.collect::<Vec<String>>()
				});

				if let Some(split) = split {
					for node in split {
						tags.push(String::from(node));
					}
				}
			}
		}
		let (content_rating, viewer) = self.category_parser(params, &tags);
		let status = (params.status_mapping)(
			(params.manga_details_status_transformer)(
				details
					.select(params.manga_details_status)
					.and_then(|v| v.text())
					.unwrap_or("".to_string()),
			)
			.to_string(),
		);
		Ok(Manga {
			key: (params.manga_parse_id)(url.clone()).to_string(),
			cover,
			title: (params.manga_details_title_transformer)(title.unwrap_or("".to_string()))
				.to_string(),
			authors,
			description,
			url: Some(url),
			tags: Some(tags),
			status,
			content_rating,
			viewer,
			..Default::default()
		})
	}

	fn get_chapter_list(&self, params: &mut Params, url: String) -> Result<Vec<Chapter>> {
		let mut skipped_first = false;
		let mut chapters: Vec<Chapter> = Vec::new();

		self.cache_manga_page(params, url.as_str())?;

		let html = self.get_cache_manga_value(params).unwrap();

		let html = Html::parse_with_url(html, url)?;
		let title_untrimmed = (params.manga_details_title_transformer)(
			html.select(params.manga_details_title)
				.and_then(|v| v.text())
				.unwrap_or("".to_string()),
		);
		let title = title_untrimmed.trim();
		for chapter_node in html.select(params.manga_details_chapters).unwrap() {
			if params.chapter_skip_first && !skipped_first {
				skipped_first = true;
				continue;
			}
			let chapter_url = chapter_node
				.select_first(params.chapter_anchor_selector)
				.unwrap()
				.attr("abs:href")
				.unwrap();

			let chapter_id = (params.chapter_parse_id)(chapter_url.clone());
			let mut chapter_title = chapter_node
				.select(params.chapter_anchor_selector)
				.unwrap()
				.text()
				.unwrap_or_default();
			let title_raw = chapter_title.clone();
			let numbers =
				extract_f32_from_string(String::from(title), String::from(&chapter_title));
			let (volume_number, chapter_number) =
				if numbers.len() > 1 && chapter_title.to_ascii_lowercase().contains("vol") {
					(numbers[0], numbers[1])
				} else if !numbers.is_empty() {
					(-1.0, numbers[0])
				} else {
					(-1.0, -1.0)
				};
			if chapter_number >= 0.0 {
				let splitter = format!(" {}", chapter_number);
				let splitter2 = format!("#{}", chapter_number);
				if chapter_title.contains(&splitter) {
					let split = chapter_title.splitn(2, &splitter).collect::<Vec<&str>>();
					chapter_title =
						String::from(split[1]).replacen(|char| char == ':' || char == '-', "", 1);
				} else if chapter_title.contains(&splitter2) {
					let split = chapter_title.splitn(2, &splitter2).collect::<Vec<&str>>();
					chapter_title =
						String::from(split[1]).replacen(|char| char == ':' || char == '-', "", 1);
				}
			}
			let date_updated = (params.time_converter)(
				&params,
				&chapter_node
					.select(params.chapter_date_selector)
					.unwrap()
					.text()
					.unwrap_or_default(),
			) as i64;

			chapter_title = chapter_title.trim().to_string();
			chapter_title = String::from(if chapter_title.is_empty() {
				title_raw.trim()
			} else {
				&chapter_title
			});

			chapters.push(Chapter {
				key: chapter_id.to_string(),
				title: if chapter_title.is_empty() {
					None
				} else {
					Some(chapter_title)
				},
				volume_number: if volume_number < 0.0 {
					None
				} else {
					Some(volume_number)
				},
				chapter_number: if chapter_number < 0.0 && volume_number >= 0.0 {
					None
				} else {
					Some(chapter_number)
				},
				date_uploaded: Some(date_updated),
				url: Some(chapter_url),
				..Default::default()
			});
		}
		Ok(chapters)
	}

	fn get_page_list(
		&self,
		params: &mut Params,
		manga: Manga,
		chapter: Chapter,
	) -> Result<Vec<Page>> {
		let mut pages: Vec<Page> = Vec::new();
		let url = (params.page_list_page)(params, &manga, &chapter);
		let html = self.create_request(params, &url, None)?.html()?;
		for page_node in html.select(params.manga_viewer_page).unwrap() {
			let mut page_url = if page_node.has_attr("data-original") {
				page_node.attr("abs:data-original")
			} else {
				None
			};

			if page_url.is_none() {
				page_url = if page_node.has_attr("data-cdn") {
					page_node.attr("abs:data-cdn")
				} else {
					page_url
				}
			}
			if page_url.is_none() {
				page_url = if page_node.has_attr("data-src") {
					page_node.attr("data-src")
				} else {
					page_url
				}
			}
			if page_url.is_none() {
				page_url = if page_node.has_attr("src") {
					page_node.attr("src")
				} else {
					page_url
				}
			}

			pages.push(Page {
				content: PageContent::Url(
					(params.page_url_transformer)(page_url.unwrap_or_default()).to_string(),
					None,
				),
				has_description: false,
				..Default::default()
			});
		}

		Ok(pages)
	}

	fn handle_deep_link(&self, params: &mut Params, url: String) -> Result<Option<DeepLinkResult>> {
		self.cache_manga_page(params, url.as_str())?;

		let html = self.get_cache_manga_value(params).unwrap();
		let html = Html::parse_with_url(html, url.clone())?;
		if html.select(params.manga_viewer_page).is_none() {
			let breadcrumbs = html.select(".breadcrumb li").unwrap();
			let manga_id = breadcrumbs
				.get(breadcrumbs.size() / 2 - 2)
				.expect("node array")
				.select_first("a")
				.unwrap()
				.attr("abs:href")
				.unwrap_or_default();
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: (params.manga_parse_id)(manga_id).to_string(),
				key: (params.chapter_parse_id)(url.clone()).to_string(),
			}))
		} else {
			Ok(Some(DeepLinkResult::Manga {
				key: (params.manga_parse_id)(url).to_string(),
			}))
		}
	}

	fn get_search_manga_list(
		&self,
		params: &mut Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = { (params.get_search_url)(params, query, page, filters)? };
		self.get_manga_list(params, url, None)
	}

	fn get_manga_update(
		&self,
		params: &mut Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = (params.manga_page)(params, &manga);

		if needs_details {
			let new_manga = self.parse_manga_element(params, url.clone())?;

			manga.cover = new_manga.cover;
			manga.title = new_manga.title;
			manga.authors = new_manga.authors;
			manga.description = new_manga.description;
			manga.url = new_manga.url;
			manga.tags = new_manga.tags;
			manga.status = new_manga.status;
			manga.content_rating = new_manga.content_rating;
			manga.viewer = new_manga.viewer;

			send_partial_result(&manga);
		}

		if needs_chapters {
			manga.chapters = Some(self.get_chapter_list(params, url)?);
		}

		Ok(manga)
	}

	fn get_home(&self, params: &mut Params) -> Result<HomeLayout> {
		let base_url = &params.base_url.clone();
		let html = { self.create_request(params, base_url, None)?.html()? };

		let mut components = Vec::new();

		let parse_manga = |el: &Element| -> Option<Manga> {
			let manga_link = el
				.select_first(params.home_manga_link)
				.or_else(|| el.select_first(".widget-title a"))?;
			Some(Manga {
				key: (params.manga_parse_id)(manga_link.attr("abs:href")?).into(),
				title: manga_link.text()?,
				cover: el.select_first("img").and_then(|img| {
					img.attr(params.home_manga_cover_attr)
						.or_else(|| img.attr("data-cfsrc"))
				}),
				url: manga_link.attr("href"),
				..Default::default()
			})
		};
		let parse_manga_with_chapter = |el: &Element| -> Option<MangaWithChapter> {
			let manga = parse_manga(el)?;
			let chapter_link = el.select_first(params.home_chapter_link)?;
			let title_text = chapter_link.text()?;
			let chapter_number = find_first_f32(&title_text);
			Some(MangaWithChapter {
				manga,
				chapter: Chapter {
					key: (params.chapter_parse_id)(chapter_link.attr("abs:href")?).into(),
					title: if title_text.contains("-") {
						title_text
							.split_once('-')
							.map(|(_, title)| title.trim().into())
					} else {
						Some(title_text)
					},
					chapter_number,
					date_uploaded: el
						.select_first(params.home_date_uploaded)
						.and_then(|el| {
							if params.home_date_uploaded_attr == "text" {
								el.text()
							} else {
								el.attr(params.home_date_uploaded_attr)
							}
						})
						.map(|date| (params.time_converter)(&params, &date)),
					url: chapter_link.attr("href"),
					..Default::default()
				},
			})
		};

		if let Some(popular_sliders) = html.select(params.home_sliders_selector) {
			for popular_slider in popular_sliders {
				let title = popular_slider
					.select_first(params.home_sliders_title_selector)
					.and_then(|el| el.text());
				let items = popular_slider
					.select(params.home_sliders_item_selector)
					.map(|els| els.filter_map(|el| parse_manga(&el)).collect::<Vec<_>>())
					.unwrap_or_default();
				if !items.is_empty() {
					components.push(HomeComponent {
						title,
						subtitle: None,
						value: aidoku::HomeComponentValue::Scroller {
							entries: items.into_iter().map(|m| m.into()).collect(),
							listing: None,
						},
					});
				}
			}
		}

		if let Some(main_cols) = html.select(params.home_grids_selector) {
			for main_col in main_cols {
				let title = main_col
					.select_first(params.home_grids_title_selector)
					.and_then(|el| el.text());
				let last_updates = main_col
					.select(params.home_grids_item_selector)
					.map(|els| {
						els.filter_map(|el| parse_manga_with_chapter(&el))
							.collect::<Vec<_>>()
					})
					.unwrap_or_default();
				if !last_updates.is_empty() {
					components.push(HomeComponent {
						title,
						subtitle: None,
						value: aidoku::HomeComponentValue::MangaChapterList {
							page_size: Some(4),
							entries: last_updates,
							listing: None,
						},
					});
				}
			}
		}

		Ok(HomeLayout { components })
	}

	fn get_dynamic_filters(&self, params: &mut Params) -> Result<Vec<Filter>> {
		let request = self.create_request(
			params,
			&format!("{}{}", params.base_url, params.genre_endpoint),
			None,
		)?;
		let html = request.html()?;

		let (options, ids) = html
			.select_first(".form-group")
			.ok_or(error!("Failed to find .form-group row"))?
			.select(".genre-item")
			.ok_or(error!("Failed to select .genre-item"))?
			.filter_map(|el| {
				let option = el.text()?;
				let id = el.select_first("span")?.attr("data-id")?;
				Some((option.into(), id.into()))
			})
			.unzip();

		Ok(vec![
			MultiSelectFilter {
				id: "category".into(),
				title: Some("Genres".into()),
				is_genre: true,
				can_exclude: false,
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		])
	}

	fn get_image_request(
		&self,
		params: &mut Params,
		url: String,
		context: Option<PageContext>,
	) -> Result<Request> {
		let mut request = {
			if let Some(context) = context
				&& let Some(referer) = context.get("Referer")
			{
				self.modify_request(params, Request::get(url)?.header("Referer", referer))?
			} else {
				self.modify_request(
					params,
					Request::get(url)?.header("Referer", &format!("{}/", params.base_url)),
				)?
			}
		};

		if let Some(user_agent) = params.user_agent {
			request = request.header("User-Agent", user_agent);
		}

		Ok(request)
	}

	fn modify_request(&self, _params: &mut Params, request: Request) -> Result<Request> {
		Ok(request)
	}
}
