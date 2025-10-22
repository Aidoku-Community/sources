use crate::{helpers, models::*, Params};
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	helpers::{element::ElementHelpers, string::StripPrefixOrSelf, uri::QueryParameters},
	imports::{html::Html, net::Request, std::send_partial_result},
	prelude::*,
	Chapter, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue, HomeLayout, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result,
};

const PER_PAGE: i32 = 18;

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = {
			let api_url = params.get_api_url();
			let mut qs = QueryParameters::new();
			qs.push("page", Some(&page.to_string()));
			qs.push("perPage", Some(&PER_PAGE.to_string()));
			if let Some(query) = query {
				qs.push("searchTerm", Some(query.trim()));
			}
			if api_url.starts_with("https://api.") {
				qs.push("tag", Some("latestUpdate"));
				qs.push("isNovel", Some("false"));
			}
			for filter in filters {
				match filter {
					FilterValue::Sort { id, index, .. } => {
						qs.push(&id, Some(&(params.get_sort_value)(index)))
					}
					FilterValue::Select { id, value } => qs.push(&id, Some(&value)),
					FilterValue::MultiSelect { included, .. } => {
						qs.push("genreIds", Some(&included.join(",")));
					}
					_ => {}
				}
			}
			format!("{api_url}/api/query?{qs}")
		};

		let mut response = Request::get(url)?
			.header("Referer", &format!("{}/", params.base_url))
			.send()?;

		let data = response.get_json::<SearchResponse>()?;
		let entries = data
			.posts
			.into_iter()
			.map(|m| m.parse_basic_manga(params))
			.collect();
		let has_next_page = data.total_count > page * PER_PAGE;

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
		let api_url = params.get_api_url();
		let url = if let Some(slug) = manga.key.strip_prefix("/series/") {
			// handle url path from home components
			format!("{api_url}/api/post?postSlug={slug}")
		} else if params.use_slug_series_keys {
			// handle slug from api
			format!("{api_url}/api/post?postSlug={}", manga.key)
		} else {
			// handle id from api
			format!("{api_url}/api/post?postId={}", manga.key)
		};

		let mut response = Request::get(url)?
			.header("Referer", &format!("{}/", params.base_url))
			.send()?;
		let data = response.get_json::<PostResponse>()?;

		if needs_details {
			manga.copy_from(data.post.parse_manga(params));
			send_partial_result(&manga);
		}

		if needs_chapters {
			manga.chapters = Some(if params.fetch_full_chapter_list {
				let mut response =
					Request::get(format!("{api_url}/api/chapters?postId={}", data.post.id))?
						.header("Referer", &format!("{}/", params.base_url))
						.send()?;
				let new_data = response.get_json::<ChaptersResponse>()?;
				new_data.post.chapters(&params.base_url, data.post.slug)
			} else {
				data.post.chapters(&params.base_url)
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/api/chapter?postId={}&chapterId={}",
			params.get_api_url(),
			manga.key,
			chapter.key
		);

		let mut response = Request::get(url)?
			.header("Referer", &format!("{}/", params.base_url))
			.send()?;
		let data = response.get_json::<ChapterResponse>()?;

		if let Some(content) = data.chapter.content.and_then(|content| {
			if content.is_empty() {
				None
			} else {
				Some(content)
			}
		}) {
			// text content
			let text = Html::parse_fragment(content)?
				.select_first("body")
				.expect("parsed fragment must have body")
				.text_with_newlines()
				.ok_or(error!("Invalid chapter content"))?;
			Ok(vec![Page {
				content: PageContent::text(text),
				..Default::default()
			}])
		} else {
			// image content
			Ok(data
				.chapter
				.images
				.map(|images| {
					images
						.into_iter()
						.map(|image| Page {
							content: PageContent::url(image.url),
							..Default::default()
						})
						.collect()
				})
				.unwrap_or_default())
		}
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = Request::get(&params.base_url)?.html()?;

		let mut components = Vec::new();

		// top header scroller
		if let Some(header) = html.select_first("main section") {
			let entries: Vec<Manga> = header
				.select(
					".swiper > .swiper-wrapper > .swiper-slide, \
					 ul > li:not(.splide__slide--clone), \
					 .embla > .embla__viewport > .embla__container > .embla__slide",
				)
				.map(|els| {
					els.filter_map(|el| {
						let title = el
							.select_first("h2")
							.or_else(|| el.select_first("h3"))
							.and_then(|h| h.text())?;
						let key = el
							.select_first("a")
							.or_else(|| {
								// the link to this series isn't contained inside the slide
								// see if we can find it elsewhere in the html
								html.select_first(format!("a[title=\"{title}\"]"))
							})
							.and_then(|a| a.attr("href"))
							.map(|href| href.strip_prefix_or_self(&params.base_url).into())
							.unwrap_or_else(|| {
								// otherwise, try and construct the slug ourselves
								format!("/series/{}", helpers::slugify(&title))
							});
						Some(Manga {
							key,
							title,
							cover: el.select_first("img").and_then(|img| img.attr("src")),
							description: el
								.select_first(".text-lg")
								.or_else(|| el.select_first("h3 + p"))
								.and_then(|text| text.text_with_newlines())
								.map(|text| text.trim().into()),
							tags: el
								.select(".flex > span, .flex > div > span")
								.map(|spans| spans.filter_map(|span| span.text()).collect()),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default();
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: None,
					subtitle: None,
					value: HomeComponentValue::BigScroller {
						entries,
						auto_scroll_interval: None,
					},
				});
			}
		}

		// "popular today" scroller
		if let Some(popular_today) = html.select_first(
			"main > div > .splide, \
			 div > .swiper, \
			 div.w-full:contains(Popular Today) > div > div[aria-roledescription=carousel] > div",
		) {
			let title = popular_today
				.parent()
				.and_then(|parent| {
					if parent.has_class("grid-slider") || parent.has_class("cinematic-slider") {
						parent.parent()
					} else {
						Some(parent)
					}
				})
				.and_then(|parent| parent.prev())
				.and_then(|sibling| sibling.select_first("h1"))
				.and_then(|h1| h1.text());
			components.push(HomeComponent {
				title,
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: popular_today
						.select(
							"ul > li:not(.splide__slide--clone) > a, \
							 .swiper-slide > a, \
							 div[aria-roledescription=slide]",
						)
						.map(|els| {
							els.filter_map(|el| {
								let key = el
									.select_first("a")?
									.attr("href")?
									.strip_prefix_or_self(&params.base_url)
									.into();
								Some(
									Manga {
										key,
										title: el
											.select_first("h1, h3")
											.and_then(|h1| h1.text())
											.unwrap_or_default(),
										cover: el.select_first("img").and_then(|img| {
											img.attr("abs:src").or_else(|| img.attr("abs:srcset"))
										}),
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
			});
		}

		if let Some(main_body) = html
			.select(
				"main > div.relative, \
				 main > div > div.relative, \
				 main section > div.w-full > .grid > div:contains(Last Updates)",
			)
			.and_then(|mut els| els.next_back())
		{
			for child in main_body.children() {
				// grid of items
				if let Some(grid) = child.select_first("div.grid.grid-cols-2, div.grid.grid-cols-1")
				{
					let title = grid
						.prev()
						.and_then(|sibling| sibling.select_first("h1"))
						.and_then(|h1| h1.text());
					components.push(HomeComponent {
						title,
						subtitle: None,
						value: HomeComponentValue::Scroller {
							entries: grid
								.children()
								.filter_map(|el| {
									let link = el.select_first("a")?;
									let key = link
										.attr("href")?
										.strip_prefix_or_self(&params.base_url)
										.into();
									Some(
										Manga {
											key,
											title: el
												.select_first("h1, a > p")
												.and_then(|h1| h1.text())
												.or_else(|| link.attr("title"))
												.unwrap_or_default(),
											cover: el.select_first("img").and_then(|img| {
												img.attr("abs:src")
													.or_else(|| img.attr("abs:srcset"))
											}),
											..Default::default()
										}
										.into(),
									)
								})
								.collect(),
							listing: None,
						},
					});
				}

				// trending list
				if let Some(list) = child.select_first(
					"div.grid.gap-3:not(.grid-cols-1), div.grid.gap-4:not(.grid-cols-1)",
				) {
					let title = list
						.parent()
						.and_then(|parent| parent.prev())
						.and_then(|sibling| sibling.select_first("h1"))
						.and_then(|h1| h1.text());
					components.push(HomeComponent {
						title,
						subtitle: None,
						value: HomeComponentValue::MangaList {
							ranking: true,
							page_size: None,
							entries: list
								.children()
								.filter_map(|el| {
									let link = el.select_first("a")?;
									let key = link
										.attr("href")?
										.strip_prefix_or_self(&params.base_url)
										.into();
									Some(
										Manga {
											key,
											title: el
												.select_first("h3, a > p")
												.and_then(|h| h.own_text())
												.or_else(|| link.attr("title"))
												.unwrap_or_default(),
											cover: el.select_first("img").and_then(|img| {
												img.attr("abs:src")
													.or_else(|| img.attr("abs:srcset"))
											}),
											..Default::default()
										}
										.into(),
									)
								})
								.collect(),
							listing: None,
						},
					});
				}
			}
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

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		const SERIES_PATH: &str = "series/";
		if !path.starts_with(SERIES_PATH) {
			return Ok(None);
		}

		if let Some(idx) = path.rfind("/chapter-") {
			// ex: https://hivetoons.org/series/true-education/chapter-199
			// we use chapter ids not slugs as keys, so just link the series not the chapter
			let manga_key = &path[..idx];
			Ok(Some(DeepLinkResult::Manga {
				key: manga_key.into(),
			}))
		} else {
			// ex: https://hivetoons.org/series/true-education
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		}
	}
}
