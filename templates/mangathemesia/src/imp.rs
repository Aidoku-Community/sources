use crate::{
	Params,
	helpers::{self, ElementImageAttr},
};
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue,
	HomeLayout, Link, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent,
	PageContext, Result, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::{string::StripPrefixOrSelf, uri::QueryParameters},
	imports::{
		html::{Document, Html},
		net::Request,
		std::{current_date, parse_date_with_options, send_partial_result},
	},
	prelude::*,
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
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		if query.is_some() {
			qs.push("title", query.as_deref());
		}

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => {
					qs.push(&id, Some(&value));
				}
				FilterValue::Sort { id, index, .. } => {
					let value = match index {
						0 => "",
						1 => "title",
						2 => "titlereverse",
						3 => "update",
						4 => "latest",
						5 => "popular",
						_ => "",
					};
					qs.push(&id, Some(value));
				}
				FilterValue::Select { id, value } => {
					qs.set(&id, Some(&value));
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					for item in included {
						qs.push(&id, Some(&item));
					}
					for item in excluded {
						qs.push(&id, Some(&format!("-{item}")));
					}
				}
				_ => {}
			}
		}

		let url = format!("{}{}/?{qs}", params.base_url, params.manga_url_directory);
		let html = Request::get(url)?.html()?;

		Ok(MangaPageResult {
			entries: html
				.select(".utao .uta .imgu, .listupd .bs .bsx, .listo .bs .bsx")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("a")?;
						Some(Manga {
							key: link
								.attr("href")?
								.strip_prefix_or_self(&params.base_url)
								.into(),
							title: link.attr("title")?,
							cover: el.select_first("img")?.img_attr(),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default(),
			has_next_page: html
				.select_first("div.pagination .next, div.hpage .r")
				.is_some(),
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{}{}", params.base_url, manga.key);
		let html = Request::get(&manga_url)?.html()?;

		if needs_details {
			let details_element = html
				.select_first("div.bigcontent, div.animefull, div.main-info, div.postbody")
				.ok_or_else(|| error!("Unable to find details"))?;

			manga.title = details_element
				.select_first(&params.series_title_selector)
				.and_then(|h1| h1.text())
				.unwrap_or(manga.title);
			manga.cover = details_element
				.select_first(&params.series_cover_selector)
				.and_then(|img| img.img_attr())
				.or(manga.cover);
			manga.artists = details_element
				.select_first(&params.series_artist_selector)
				.and_then(|el| el.own_text())
				.and_then(|text| {
					if text.is_empty() || text == "-" || text == "N/A" || text == "n/a" {
						None
					} else {
						Some(vec![text])
					}
				});
			manga.authors = details_element
				.select_first(&params.series_author_selector)
				.and_then(|el| el.own_text())
				.and_then(|text| {
					if text.is_empty() || text == "-" || text == "N/A" || text == "n/a" {
						None
					} else {
						Some(vec![text])
					}
				});
			manga.description = self.parse_description(params, &html);
			manga.url = Some(manga_url.clone());
			manga.tags = details_element
				.select(&params.series_genre_selector)
				.map(|els| {
					els.filter_map(|el| el.text())
						.map(|s| s.trim().into())
						.collect()
				});
			manga.status = details_element
				.select_first(&params.series_status_selector)
				.and_then(|el| el.text())
				.map(|text| self.get_manga_status(&text))
				.unwrap_or_default();
			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating =
				if params.mark_all_nsfw || html.select_first(".restrictcontainer").is_some() {
					ContentRating::NSFW
				} else if tags.iter().any(|e| e == "Ecchi") {
					ContentRating::Suggestive
				} else {
					ContentRating::Safe
				};
			manga.viewer = details_element
				.select_first(&params.series_type_selector)
				.and_then(|el| el.text())
				.map(|text| match text.to_lowercase().as_str() {
					"manga" | "one-shot" | "oneshot" | "doujinshi" => Viewer::RightToLeft,
					"manhua" | "manhwa" => Viewer::Webtoon,
					"comic" => Viewer::LeftToRight,
					_ => Viewer::Unknown,
				})
				.unwrap_or_default();

			send_partial_result(&manga);
		}

		if needs_chapters {
			manga.chapters = html.select(&params.chapter_list_selector).map(|els| {
				els.filter_map(|el| {
					let link = el.select_first("a")?;
					let url = link.attr("abs:href")?;
					let title = link
						.select_first(".lch a, .chapternum")
						.and_then(|el| {
							let text = el.text()?;
							if !text.is_empty() { Some(text) } else { None }
						})
						.or_else(|| link.text())?;
					let chapter_number = helpers::find_first_f32(&title);
					Some(Chapter {
						key: url.strip_prefix_or_self(&params.base_url).into(),
						title: if title.as_str()
							!= format!("Chapter {}", chapter_number.unwrap_or(0.0))
						{
							Some(title)
						} else {
							None
						},
						chapter_number,
						date_uploaded: Some(
							el.select_first(".chapterdate")
								.and_then(|el| el.text())
								.and_then(|text| {
									parse_date_with_options(
										text,
										&params.date_format,
										&params.date_locale,
										"current",
									)
								})
								.unwrap_or_else(current_date),
						),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			});
		}

		Ok(manga)
	}

	fn parse_description(&self, params: &Params, html: &Document) -> Option<String> {
		html.select_first("div.bigcontent, div.animefull, div.main-info, div.postbody")
			.and_then(|el| el.select_first(&params.series_description_selector))
			.and_then(|div| div.text())
	}

	fn get_manga_status(&self, str: &str) -> MangaStatus {
		match str.to_lowercase().as_str() {
			"en curso"
			| "ongoing"
			| "on going"
			| "ativo"
			| "en cours"
			| "en cours de publication"
			| "đang tiến hành"
			| "em lançamento"
			| "онгоінг"
			| "publishing"
			| "devam ediyor"
			| "em andamento"
			| "in corso"
			| "güncel"
			| "berjalan"
			| "продолжается"
			| "updating"
			| "lançando"
			| "in arrivo"
			| "emision"
			| "en emision"
			| "curso"
			| "en marcha"
			| "publicandose"
			| "publicando"
			| "连载中"
			| "連載中"
			| "مستمرة"
			| "مستمر"
			| "devam etmekte" => MangaStatus::Ongoing,
			"completed" | "completo" | "complété" | "fini" | "achevé" | "terminé"
			| "tamamlandı" | "đã hoàn thành" | "hoàn thành" | "завершено" | "finished"
			| "finalizado" | "completata" | "one-shot" | "bitti" | "tamat" | "completado"
			| "concluído" | "完結" | "concluido" | "已完结" | "مكتملة" | "bitmiş" => {
				MangaStatus::Completed
			}
			"canceled" | "cancelled" | "cancelado" | "cancellato" | "cancelados" | "dropped"
			| "discontinued" | "abandonné" => MangaStatus::Cancelled,
			"hiatus" | "on hold" | "pausado" | "en espera" | "en pause" | "en attente"
			| "hiato" => MangaStatus::Hiatus,
			_ => MangaStatus::Unknown,
		}
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}{}", params.base_url, chapter.key);
		let response = Request::get(&url)?.string()?;
		let html = Html::parse_fragment_with_url(&response, &url)?;

		let pages: Vec<Page> = html
			.select("div#readerarea img")
			.map(|els| {
				els.filter_map(|el| {
					Some(Page {
						content: PageContent::url(el.img_attr()?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		if !pages.is_empty() {
			return Ok(pages);
		}

		if response.contains("\"images\":") {
			Ok(helpers::extract_images(&response)
				.into_iter()
				.map(|url| Page {
					content: PageContent::url(url),
					..Default::default()
				})
				.collect())
		} else {
			bail!("No pages found")
		}
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Accept", "image/avif,image/webp,image/png,image/jpeg,*/*")
			.header("Referer", &format!("{}/", params.base_url)))
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = Request::get(&params.base_url)?.html()?;

		let mut components = Vec::new();

		if let Some(hero) = html.select_first("div.big-slider, div.slidtop") {
			let entries: Vec<Manga> = hero
				.select(
					".swiper-wrapper > .swiper-slide:not(.swiper-slide-duplicate), .owl-carousel > .slide-item",
				)
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
								.select_first("a span")
								.or_else(|| el.select_first(".title a"))
								.and_then(|el| el.text())
								.unwrap_or_default(),
							cover: el
								.select_first("img")
								.and_then(|img| img.img_attr())
								.or_else(|| {
									el.select_first(".bigbanner").and_then(|el| {
										let url = if let Some(url) = el.attr("data-bg") {
											Some(url)
										} else {
											let style = el.attr("style")?;
											helpers::extract_between(&style, "url('", "')")
												.map(|s| s.into())
										};
										if let Some(url) = url.as_ref()
											&& url.starts_with('/')
										{
											Some(format!("{}{url}", params.base_url))
										} else {
											url
										}
									})
								}),
							description: link.select_first("div").and_then(|el| el.text()).or_else(
								|| {
									el.select_first(".desc > p, .excerpt > p:not(.story)")
										.and_then(|el| el.text())
								},
							),
							tags: el
								.select("span:not(.ellipsis) > a, .extra-category > a")
								.map(|els| els.filter_map(|el| el.text()).collect())
								.and_then(|tags: Vec<String>| {
									if tags.is_empty() {
										el.select_first("span:contains(Genres)")
											.or_else(|| el.select_first(".type-genre > span"))
											.and_then(|el| el.text())
											.map(|s| {
												s.split_once(": ")
													.unwrap_or(("", &s))
													.1
													.split(",")
													.map(Into::into)
													.collect()
											})
									} else {
										Some(tags)
									}
								}),
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
						auto_scroll_interval: Some(5.0),
					},
				});
			}
		}

		if let Some(popular_today) =
			html.select_first("div.popularslider, .hothome > .row, .hothome > .listupd")
		{
			let title = popular_today
				.parent()
				.and_then(|el| el.select_first(".releases > h2"))
				.and_then(|el| el.text())
				.unwrap_or("Popular Today".into());
			let entries: Vec<Link> = popular_today
				.select(".popconslide > .bs")
				.and_then(|els| {
					if els.is_empty() {
						popular_today.select(".listupd .bsx")
					} else {
						Some(els)
					}
				})
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
								title: link.attr("title")?,
								cover: el.select_first("img").and_then(|img| img.img_attr()),
								..Default::default()
							}
							.into(),
						)
					})
					.collect()
				})
				.unwrap_or_default();
			if !entries.is_empty() {
				components.push(HomeComponent {
					title: Some(title),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries,
						listing: None,
					},
				});
			}
		}

		if let Some(chapter_lists) = html.select(".postbody > .bixbox") {
			for list in chapter_lists {
				let Some(title) = list
					.select_first("h2")
					.and_then(|h2| h2.own_text())
					.map(|s| s.trim().into())
				else {
					continue;
				};
				let entries: Vec<MangaWithChapter> = list
					.select(".listupd > .bs, .listupd > .stylesven, .listupd > .utao")
					.map(|els| {
						els.filter_map(|el| {
							let link = el.select_first("a")?;
							let chapter_link = el.select_first("ul > li a, .adds > a")?;
							let manga_key = link
								.attr("href")?
								.strip_prefix_or_self(&params.base_url)
								.into();
							let chapter_key = chapter_link
								.attr("href")?
								.strip_prefix_or_self(&params.base_url)
								.into();
							let chapter_title = chapter_link
								.select_first("span.fivchap")
								.unwrap_or(chapter_link)
								.text()?;
							let chapter_number = helpers::find_first_f32(&chapter_title);
							Some(MangaWithChapter {
								manga: Manga {
									key: manga_key,
									title: link.attr("title")?.trim().into(),
									cover: el.select_first("img").and_then(|img| img.img_attr()),
									..Default::default()
								},
								chapter: Chapter {
									key: chapter_key,
									title: chapter_number.is_none().then_some(chapter_title),
									chapter_number,
									..Default::default()
								},
							})
						})
						.collect()
					})
					.unwrap_or_default();
				if !entries.is_empty() {
					components.push(HomeComponent {
						title: Some(title),
						subtitle: None,
						value: HomeComponentValue::MangaChapterList {
							page_size: None,
							entries,
							listing: None,
						},
					});
				} else {
					let entries: Vec<Link> = list
						.select(".listupd > div > .bs")
						.map(|els| {
							els.filter_map(|el| {
								let link = el.select_first("a")?;
								let manga_key = link
									.attr("href")?
									.strip_prefix_or_self(&params.base_url)
									.into();
								Some(
									Manga {
										key: manga_key,
										title: link.attr("title")?,
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
						.unwrap_or_default();
					if !entries.is_empty() {
						components.push(HomeComponent {
							title: Some(title),
							subtitle: None,
							value: HomeComponentValue::Scroller {
								entries,
								listing: None,
							},
						});
					}
				}
			}
		}

		Ok(HomeLayout { components })
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		// try to fetch a manga using the provided path as a key
		let manga = self.get_manga_update(
			params,
			Manga {
				key: path.into(),
				..Default::default()
			},
			true,
			false,
		);

		// if the fetch was successful, this is a valid deep link
		if manga.is_ok_and(|manga| !manga.title.is_empty()) {
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		} else {
			Ok(None)
		}
	}
}
