use super::{helper::ElementImageAttr, parser, Params};
use aidoku::{
	alloc::{
		borrow::{Cow, ToOwned},
		String, Vec,
	},
	helpers::uri::{encode_uri_component, QueryParameters},
	imports::{
		canvas::ImageRef,
		error::AidokuError,
		html::{Element, Html},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
	Chapter, ContentRating, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue,
	HomeLayout, ImageResponse, Listing, Manga, MangaPageResult, MangaStatus, MangaWithChapter,
	Page, PageContent, PageContext, Result, Viewer,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	// css selector for chapter list items (typically contained in #{lang}-chapters or #{lang}-chaps)
	fn get_chapter_selector(&self) -> Cow<'static, str> {
		"#en-chapters > li".into()
	}

	// the language of a chapter
	fn get_chapter_language(&self, _element: &Element) -> String {
		"en".into()
	}

	// path added to base url for page list ajax request
	fn get_page_url_path(&self, chapter_id: &str) -> String {
		format!("//ajax/image/list/{chapter_id}?mode=vertical")
	}

	fn set_default_filters(&self, _query_params: &mut QueryParameters) {}

	fn get_sort_id(&self, index: i32) -> Cow<'static, str> {
		match index {
			0 => "default",
			1 => "latest-updated",
			2 => "score",
			3 => "name-az",
			4 => "release-date",
			5 => "most-viewed",
			_ => "default",
		}
		.into()
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = if let Some(query) = query {
			format!(
				"{}{}?{}={}&{}={page}",
				params.base_url,
				params.search_path,
				params.search_param,
				encode_uri_component(query),
				params.page_param
			)
		} else {
			let mut qs = QueryParameters::new();
			self.set_default_filters(&mut qs);
			for filter in filters {
				match filter {
					FilterValue::Sort { index, .. } => {
						qs.set("sort", Some(self.get_sort_id(index).as_ref()));
					}
					FilterValue::Select { id, value } => {
						qs.set(&id, Some(&value));
					}
					// genres
					FilterValue::MultiSelect { included, .. } => {
						qs.set("genres", Some(&included.join(",")));
					}
					_ => {}
				}
			}
			format!(
				"{}/filter?{}={page}{}{qs}",
				params.base_url,
				params.page_param,
				if !qs.is_empty() { "&" } else { "" }
			)
		};
		let html = Request::get(&url)?.html()?;

		let entries = parser::parse_response(
			&html,
			params.base_url.as_ref(),
			".manga_list-sbs .manga-poster",
		);

		let has_next_page = html
			.select_first("ul.pagination > li.active + li")
			.is_some();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}{}", params.base_url, manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			let element = html
				.select_first("#ani_detail")
				.ok_or(AidokuError::message("Unable to find manga details"))?;

			manga.title = element
				.select_first(".manga_name")
				.and_then(|e| e.own_text())
				.unwrap_or(manga.title.clone());
			manga.cover = element.select_first("img").and_then(|img| img.img_attr());

			let (authors, artists) = element
				.select(".anisc-info > .item:contains(Author), .anisc-info > .item:contains(著者)")
				.and_then(|authors_element| {
					let text = authors_element.text()?;
					let author_names = authors_element.select("a")?.filter_map(|el| el.own_text());

					let mut authors = Vec::new();
					let mut artists = Vec::new();
					for author in author_names {
						let is_artist = text.contains(&format!("{author} (Art)"));
						let name = author.replace(",", "");
						if is_artist {
							artists.push(name);
						} else {
							authors.push(name);
						}
					}
					Some((Some(authors), Some(artists)))
				})
				.unwrap_or((None, None));
			manga.authors = authors;
			manga.artists = artists;

			manga.description = element
				.select_first(".description")
				.and_then(|e| e.own_text());
			manga.url = Some(url);
			manga.tags = element
				.select(".genres > a")
				.map(|els| els.filter_map(|el| el.own_text()).collect());
			manga.status = element
				.select_first(".anisc-info > .item:contains(Status) .name, .anisc-info > .item:contains(地位) .name")
				.and_then(|el| el.text())
				.map(|status| match status.to_lowercase().as_str() {
					"ongoing" | "publishing" | "releasing" => MangaStatus::Ongoing,
					"completed" | "finished" => MangaStatus::Completed,
					"on-hiatus" | "on hiatus" => MangaStatus::Hiatus,
					"canceled" | "discontinued" => MangaStatus::Cancelled,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();

			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags.iter().any(|e| e == "Hentai" || e == "エロい") {
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else if element
				.select_first(".anisc-info > .item:contains(タイプ) .name")
				.and_then(|el| el.text())
				.is_some_and(|t| t == "オトナコミック")
			{
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			};

			manga.viewer = element
				.select_first(".anisc-info > .item:contains(Type) .name")
				.and_then(|el| el.text())
				.map(|status| match status.to_lowercase().as_str() {
					"manhwa" | "manhua" => Viewer::Webtoon,
					"comic" => Viewer::LeftToRight,
					_ => Viewer::RightToLeft,
				})
				.unwrap_or(Viewer::RightToLeft);

			send_partial_result(&manga);
		}

		if needs_chapters {
			manga.chapters = html.select(self.get_chapter_selector()).map(|els| {
				let mut c = els
					.filter_map(|el| {
						let link = el.select_first("a")?;
						let url = link.attr("abs:href")?;
						let mut key: String = url.strip_prefix(params.base_url.as_ref())?.into();
						if let Some(id) = el.attr("data-id") {
							key.push_str(&format!("#{id}"));
						}
						let mut title = link.select_first(".name").and_then(|el| el.text());
						let chapter_number = title
							.as_ref()
							.and_then(|title| title.find(':'))
							.and_then(|colon| {
								let chapter_num_text = &title.as_ref().unwrap()[..colon].to_owned();
								title = Some(title.as_ref().unwrap()[colon + 1..].trim().into());
								chapter_num_text
									.chars()
									.filter(|c| c.is_ascii_digit() || *c == '.')
									.collect::<String>()
									.parse::<f32>()
									.ok()
							});
						if title.as_ref().is_some_and(|t| {
							*t == format!("Chapter {}", chapter_number.unwrap_or_default())
								|| *t == format!("第{}話", chapter_number.unwrap_or_default())
								|| *t == format!("第 {} 話", chapter_number.unwrap_or_default())
								|| *t == format!("【第 {} 話】", chapter_number.unwrap_or_default())
						}) {
							title = None;
						}
						let language = self.get_chapter_language(&el);
						Some(Chapter {
							key,
							title,
							chapter_number,
							url: Some(url),
							language: language.into(),
							..Default::default()
						})
					})
					.collect::<Vec<_>>();
				// sort combined chapters by chapter number
				// since separate languages are grouped together by default
				c.sort_by(|a, b| {
					let a_num = a.chapter_number.unwrap_or(-1.0);
					let b_num = b.chapter_number.unwrap_or(-1.0);
					b_num
						.partial_cmp(&a_num)
						.unwrap_or(core::cmp::Ordering::Equal)
				});
				c
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let hash_pos = chapter.key.rfind('#');
		let id: Option<String> = hash_pos
			.map(|pos| (&chapter.key[pos + 1..]).into())
			.or_else(|| {
				// get chapter id from chapter page html
				Request::get(format!("{}{}", params.base_url, chapter.key))
					.and_then(|req| req.html())
					.ok()
					.and_then(|html| html.select_first("div[data-reading-id]"))
					.and_then(|el| el.attr("data-reading-id"))
			});
		let Some(id) = id else {
			bail!("Unable to retrieve chapter id");
		};

		let chapter_key_without_id = hash_pos
			.map(|pos| (&chapter.key[..pos]).into())
			.unwrap_or(chapter.key);

		let url = format!("{}{}", params.base_url, self.get_page_url_path(&id));
		let json = Request::get(url)?
			.header("Accept", "application/json, text/javascript, */*; q=0.01")
			.header(
				"Referer",
				&format!("{}{}", params.base_url, chapter_key_without_id),
			)
			.header("X-Requested-With", "XMLHttpRequest")
			.json_owned::<serde_json::Value>()?;
		let html_text = json["html"].as_str().unwrap_or_default();
		let html = Html::parse_fragment(html_text).expect("what");

		Ok(html
			.select(&params.page_selector)
			.map(|els| {
				els.filter_map(|el| {
					let url = el
						.img_attr()
						.or_else(|| el.select_first("img").and_then(|img| img.img_attr()))?;
					Some(Page {
						content: if el.has_class("shuffled") {
							let mut context = PageContext::default();
							context.insert("shuffled".into(), "1".into());
							PageContent::url_context(url.trim(), context)
						} else {
							PageContent::url(url.trim())
						},
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default())
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let url = format!(
			"{}/{}?{}={page}",
			params.base_url, listing.id, params.page_param
		);
		let html = Request::get(url)?.html()?;
		let entries = html
			.select(".item")
			.map(|els| {
				els.filter_map(|e| {
					let link_href = e.select_first("a.manga-poster")?.attr("href")?;
					Some(Manga {
						key: link_href
							.strip_prefix(params.base_url.as_ref())
							.map(|s| s.into())
							.unwrap_or(link_href),
						title: e.select_first(".manga-name")?.text()?,
						cover: e.select_first(".manga-poster img")?.attr("src"),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		Ok(MangaPageResult {
			entries,
			has_next_page: html.select_first("a.page-link[title=\"Next\"]").is_some(),
		})
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = Request::get(format!("{}/home", params.base_url))?.html()?;

		let mut components = Vec::new();

		// header
		if let Some(slider_elements) =
			html.select("#slider .deslide-item:not(.swiper-slide-duplicate)")
		{
			components.push(HomeComponent {
				value: HomeComponentValue::BigScroller {
					entries: slider_elements
						.filter_map(|el| {
							let link = el.select_first(".desi-head-title a")?;
							let link_href = link.attr("href")?;
							Some(Manga {
								key: link_href
									.strip_prefix(params.base_url.as_ref())
									.map(|s| s.into())
									.unwrap_or(link_href),
								title: link.attr("title")?,
								cover: el
									.select_first(".deslide-poster img")
									.and_then(|e| e.attr("src")),
								description: el
									.select_first(".sc-detail > .scd-item")
									.and_then(|e| e.text()),
								tags: el
									.select(".sc-detail > .scd-genres > span")
									.map(|els| els.filter_map(|e| e.text()).collect()),
								..Default::default()
							})
						})
						.collect(),
					auto_scroll_interval: Some(5.0),
				},
				..Default::default()
			});
		}

		fn parse_swiper(params: &Params, section: Element) -> HomeComponent {
			HomeComponent {
				title: section.select(".cat-heading").and_then(|e| e.text()),
				value: HomeComponentValue::Scroller {
					entries: section
						.select(".swiper-slide")
						.map(|els| {
							els.filter_map(|e| {
								let link_href = e.select_first(".manga-poster a")?.attr("href")?;
								Some(
									Manga {
										key: link_href
											.strip_prefix(params.base_url.as_ref())
											.map(|s| s.into())
											.unwrap_or(link_href),
										title: e
											.select_first(".anime-name, .manga-name")?
											.text()?,
										cover: e.select_first(".manga-poster img")?.attr("src"),
										..Default::default()
									}
									.into(),
								)
							})
							.collect()
						})
						.unwrap_or_default(),
					listing: None,
				},
				..Default::default()
			}
		}

		// trending
		if let Some(section) = html.select_first("#manga-trending") {
			components.push(parse_swiper(params, section));
		}

		// recommended
		if let Some(section) = html.select_first("#manga-featured") {
			components.push(parse_swiper(params, section));
		}

		// latest updates
		if let Some(section) = html.select_first("#main-content") {
			components.push(HomeComponent {
				title: section.select(".cat-heading").and_then(|e| e.text()),
				value: HomeComponentValue::MangaChapterList {
					page_size: None,
					entries: section
						.select(".item")
						.map(|els| {
							els.take(10) // limit to 10, since that's as much as the page displays initially
								.filter_map(|e| {
									let link_href =
										e.select_first("a.manga-poster")?.attr("href")?;
									let chapter_link = e.select_first(".fd-list .chapter a")?;
									let chapter_link_href = chapter_link.attr("href")?;
									Some(MangaWithChapter {
										manga: Manga {
											key: link_href
												.strip_prefix(params.base_url.as_ref())
												.map(|s| s.into())
												.unwrap_or(link_href),
											title: e.select_first(".manga-name")?.text()?,
											cover: e.select_first(".manga-poster img")?.attr("src"),
											..Default::default()
										},
										chapter: Chapter {
											key: chapter_link_href
												.strip_prefix(params.base_url.as_ref())
												.map(|s| s.into())
												.unwrap_or(chapter_link_href),
											chapter_number: chapter_link
												.text()?
												.chars()
												.filter(|c| c.is_ascii_digit() || *c == '.')
												.collect::<String>()
												.parse::<f32>()
												.ok(),
											..Default::default()
										},
									})
								})
								.collect()
						})
						.unwrap_or_default(),
					listing: None,
				},
				..Default::default()
			});
		}

		if let Some(sidebar_sections) = html.select("#main-sidebar > section") {
			for section in sidebar_sections {
				let is_ranked = section.select_first("#chart-today").is_some();
				let Some(elements) = section.select(if is_ranked {
					"#chart-today .featured-block-ul > ul > li"
				} else {
					".featured-block-ul > ul > li"
				}) else {
					continue;
				};
				if !elements.is_empty() {
					components.push(HomeComponent {
						title: section.select(".cat-heading").and_then(|e| e.text()),
						value: HomeComponentValue::MangaList {
							ranking: is_ranked,
							page_size: Some(5),
							entries: elements
								.filter_map(|e| {
									Some(
										Manga {
											key: e
												.select_first("a.manga-poster")?
												.attr("abs:href")?
												.strip_prefix(params.base_url.as_ref())?
												.into(),
											title: e.select_first(".manga-name")?.text()?,
											cover: e.select_first(".manga-poster img")?.attr("src"),
											..Default::default()
										}
										.into(),
									)
								})
								.collect(),
							listing: None,
						},
						..Default::default()
					});
				}
			}
		}

		if let Some(completed_section) =
			html.select_first("#main-wrapper > div.container > div > section")
		{
			components.push(parse_swiper(params, completed_section));
		}

		Ok(HomeLayout { components })
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{}/", params.base_url)))
	}

	fn process_page_image(
		&self,
		_params: &Params,
		_response: ImageResponse,
		_context: Option<PageContext>,
	) -> Result<ImageRef> {
		Err(AidokuError::Unimplemented)
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		const READ_PATH: &str = "read/";

		if path.starts_with(READ_PATH) {
			// ex: https://mangareader.to/read/the-weakest-job-becomes-the-strongest-in-the-world-with-past-life-knowledge-67999/en/chapter-2
			let end = path.find('/').unwrap_or(path.len());
			let manga_key = &path[..end];
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: path.into(),
			}))
		} else {
			// ex: https://mangareader.to/the-weakest-job-becomes-the-strongest-in-the-world-with-past-life-knowledge-67999
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		}
	}
}
