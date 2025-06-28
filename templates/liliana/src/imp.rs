use super::Params;
use crate::{
	helpers::{self, ElementImageAttr},
	models::*,
};
use aidoku::{
	alloc::{vec, String, Vec},
	helpers::{
		string::StripPrefixOrSelf,
		uri::{encode_uri_component, QueryParameters},
	},
	imports::{
		html::{Element, Html},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
	Chapter, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue, HomeLayout, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, PageContext, Result,
};

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
		if params.uses_post_search {
			if let Some(query) = query {
				let body = format!("search={}", encode_uri_component(query),);
				let json = Request::post(format!("{}/ajax/search", params.base_url))?
					.header("Accept", "application/json, text/javascript, *//*; q=0.01")
					.header("Host", helpers::url_host(&params.base_url))
					.header("Origin", &params.base_url)
					.header("X-Requested-With", "XMLHttpRequest")
					.body(body)
					.json_owned::<SearchResponse>()?;
				return Ok(MangaPageResult {
					entries: json
						.list
						.into_iter()
						.map(|m| m.into_manga(&params.base_url))
						.collect(),
					has_next_page: false,
				});
			}
		}

		let url = if let Some(query) = query {
			format!("{}/search/{page}/?keyword={query}", params.base_url)
		} else {
			let mut qs = QueryParameters::new();
			for filter in filters {
				match filter {
					FilterValue::Sort { id, index, .. } => {
						let value = match index {
							0 => "default",
							1 => "latest-updated",
							2 => "views",
							3 => "views_month",
							4 => "views_week",
							5 => "views_day",
							6 => "score",
							7 => "az",
							8 => "za",
							9 => "chapters",
							10 => "new",
							11 => "old",
							_ => "default",
						};
						qs.push(&id, Some(value));
					}
					FilterValue::Select { id, value } => {
						qs.push(&id, Some(&value));
					}
					FilterValue::MultiSelect {
						included, excluded, ..
					} => {
						if !included.is_empty() {
							qs.push("genres", Some(&included.join(",")));
						}
						if !excluded.is_empty() {
							qs.push("notGenres", Some(&excluded.join(",")));
						}
					}
					_ => {}
				}
			}
			format!(
				"{}/filter/{page}/{}{qs}",
				params.base_url,
				if qs.is_empty() { "" } else { "?" }
			)
		};
		let html = Request::get(url)?
			.header("Referer", &format!("{}/", params.base_url))
			.html()?;
		Ok(helpers::parse_manga_page(&html, &params.base_url))
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{}{}", params.base_url, manga.key);
		let html = Request::get(&url)?
			.header("Referer", &format!("{}/", params.base_url))
			.html()?;

		if needs_details {
			manga.title = html
				.select_first(".a2 header h1")
				.and_then(|h1| h1.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first(".a1 > figure img")
				.and_then(|img| img.img_attr())
				.or(manga.cover);
			manga.authors = html
				.select_first("div.y6x11p i.fas.fa-user + span.dt")
				.and_then(|span| span.text())
				.and_then(|text| {
					if text == "updating" {
						None
					} else {
						Some(vec![text])
					}
				});
			manga.description = html
				.select_first("div#syn-target")
				.and_then(|div| div.text());
			manga.tags = html
				.select(".a2 div > a[rel='tag'].label")
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.url = Some(url);
			manga.status = html
				.select_first("div.y6x11p i.fas.fa-rss + span.dt")
				.and_then(|span| span.text())
				.map(|text| match text.to_lowercase().as_str() {
					"ongoing" | "đang tiến hành" | "進行中" => MangaStatus::Ongoing,
					"completed" | "hoàn thành" | "完了" => MangaStatus::Completed,
					"on-hold" | "tạm ngưng" | "保留" => MangaStatus::Hiatus,
					"canceled" | "đã huỷ" | "キャンセル" => MangaStatus::Cancelled,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();
			send_partial_result(&manga);
		}

		if needs_chapters {
			manga.chapters = html.select("ul > li.chapter").map(|els| {
				els.filter_map(|el| {
					let a = el.select_first("a")?;
					let link = a.attr("abs:href")?;
					let title = a.text()?;
					Some(Chapter {
						key: link.strip_prefix_or_self(&params.base_url).into(),
						title: title.split_once('-').map(|(_, title)| title.trim().into()),
						chapter_number: helpers::find_first_f32(&title),
						date_uploaded: el
							.select_first("time")
							.and_then(|time| time.attr("datetime"))
							.and_then(|datetime| datetime.parse().ok()),
						url: Some(link),
						..Default::default()
					})
				})
				.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}", params.base_url, chapter.key);
		let html = Request::get(url)?
			.header("Referer", &format!("{}/", params.base_url))
			.html()?;

		let chapter_id: String = html
			.select("body > script:not([src])")
			.and_then(|els| {
				// find first script that contains CHAPTER_ID
				for el in els {
					if let Some(data) = el.data() {
						if data.contains("CHAPTER_ID") {
							return Some(data);
						}
					}
				}
				None
			})
			.and_then(|data| {
				helpers::extract_between(&data, "const CHAPTER_ID = ", ";").map(|s| s.into())
			})
			.ok_or(error!("Failed to get chapter id"))?;

		let url = format!("{}/ajax/image/list/chap/{chapter_id}", params.base_url);
		let data = Request::get(url)?
			.header("Accept", "application/json, text/javascript, *//*; q=0.01")
			.header("Host", helpers::url_host(&params.base_url))
			.header("Referer", &format!("{}/", params.base_url))
			.header("X-Requested-With", "XMLHttpRequest")
			.json_owned::<PageListResponse>()?;

		if !data.status {
			bail!("{}", data.msg.unwrap_or_default())
		}

		let pages_html = Html::parse_fragment(data.html)?;

		if pages_html
			.select_first("div.separator[data-index]")
			.is_none()
		{
			Ok(pages_html
				.select("div.separator")
				.map(|els| {
					els.filter_map(|el| {
						let url = el.select_first("a")?.attr("abs:href")?;
						Some(Page {
							content: PageContent::url(url),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default())
		} else {
			Ok(pages_html
				.select("div.separator[data-index]")
				.map(|els| {
					let mut indexed_pages: Vec<(i32, Page)> = els
						.filter_map(|el| {
							let index: i32 = el.attr("data-index")?.parse().ok()?;
							let url = el.select_first("a")?.attr("abs:href")?;
							Some((
								index,
								Page {
									content: PageContent::url(url),
									..Default::default()
								},
							))
						})
						.collect();
					// sort the pages by index
					indexed_pages.sort_by_key(|(index, _)| *index);
					indexed_pages.into_iter().map(|(_, page)| page).collect()
				})
				.unwrap_or_default())
		}
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: aidoku::Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let url = format!("{}/{}/{page}/", params.base_url, listing.id);
		let html = Request::get(&url)?.html()?;
		Ok(helpers::parse_manga_page(&html, &params.base_url))
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = Request::get(&params.base_url)?.html()?;

		let mut components = Vec::new();

		fn parse_scroller(element: Element, base_url: &str) -> HomeComponent {
			HomeComponent {
				title: element
					.select_first("h2, h3, h1 > span")
					.and_then(|h| h.text()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: element
						.select(".swiper .swiper-slide:not(.swiper-slide-duplicate), figure, .grid > div")
						.map(|els| {
							els.filter_map(|el| {
								let key = el
									.select_first("a")?
									.attr("href")?
									.strip_prefix_or_self(base_url)
									.into();
								Some(
									Manga {
										key,
										title: el
											.select_first(".text-center > a, figcaption > a")
											.and_then(|a| a.text())
											.unwrap_or_default(),
										cover: el
											.select_first("img")
											.and_then(|img| img.img_attr()),
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
			}
		}

		if let Some(hero) = html.select_first("#hero") {
			let title = hero.select_first("h2").and_then(|h2| h2.text());
			components.push(HomeComponent {
				title,
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: hero
						.select(".slides > .slider-item")
						.map(|els| {
							els.filter_map(|el| {
								let link = el.select_first("a")?;
								let key = link
									.attr("href")?
									.strip_prefix_or_self(&params.base_url)
									.into();
								Some(Manga {
									key,
									title: el
										.select_first(".desi-head-title")
										.and_then(|h| h.text())
										.unwrap_or_default(),
									cover: el.select_first("img").and_then(|img| img.img_attr()),
									description: el
										.select_first(".sc-detail > .scd-item")
										.and_then(|el| el.text()),
									tags: el.select_first(".sc-detail > .scd-genres").map(|el| {
										el.children().filter_map(|el| el.text()).collect()
									}),
									..Default::default()
								})
							})
							.collect()
						})
						.unwrap_or_default(),
					auto_scroll_interval: Some(5.0),
				},
			});
		}

		if let Some(hot) = html.select_first("#pin-manga") {
			components.push(parse_scroller(hot, &params.base_url));
		}

		if let Some(trend) = html.select_first("#recommend") {
			components.push(parse_scroller(trend, &params.base_url));
		}

		if let Some(feed) = html.select_first("#feed") {
			if let Some(tabs) = feed.select("h1 > span") {
				for tab in tabs {
					let Some(selector) = tab.attr("data-tab") else {
						continue;
					};
					if let Some(element) = feed.select_first(selector) {
						let mut component = parse_scroller(element, &params.base_url);
						component.title = tab.text();
						components.push(component);
					}
				}
			}
		}

		if let Some(ranking) = html.select_first("#sidebar") {
			let title = ranking.select_first("h2").and_then(|h2| h2.text());
			components.push(HomeComponent {
				title,
				subtitle: None,
				value: HomeComponentValue::MangaList {
					ranking: true,
					page_size: None,
					entries: ranking
						.select("#series-day > article")
						.map(|els| {
							els.filter_map(|el| {
								let link = el.select_first("a")?;
								let key = link
									.attr("href")?
									.strip_prefix_or_self(&params.base_url)
									.into();
								Some(
									Manga {
										key,
										title: el
											.select_first("h3")
											.and_then(|h| h.text())
											.unwrap_or_default(),
										cover: el
											.select_first("img")
											.and_then(|img| img.img_attr()),
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

		const MANGA_PATH: &str = "/manga/";
		if !path.starts_with(MANGA_PATH) {
			return Ok(None);
		}

		let slash_count = path.matches('/').count();

		if slash_count > 3 || (slash_count == 3 && !path.ends_with('/')) {
			// ex: https://rawkuro.net/manga/za-yong-fu-yu-shu-shiga-zi-fenno-zui-qiangni-qi-fukumade002/di41-3hua
			let idx = path.rfind('/').unwrap_or(0);
			let manga_key = &path[..idx];
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: path.into(),
			}))
		} else {
			// ex: https://rawkuro.net/manga/za-yong-fu-yu-shu-shiga-zi-fenno-zui-qiangni-qi-fukumade002
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		}
	}
}
