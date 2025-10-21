use super::Params;
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, HomeComponent, HomeComponentValue,
	HomeLayout, Listing, Manga, MangaPageResult, MangaWithChapter, Page, PageContent, Result,
	Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::{string::StripPrefixOrSelf, uri::QueryParameters},
	imports::{
		html::Html,
		net::Request,
		std::{parse_date, send_partial_result},
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
		qs.push("query", query.as_deref());
		qs.push("inclusion", Some("OR"));
		qs.push("exclusion", Some("OR"));

		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					let value = match index {
						0 => "popular",
						1 => "rating",
						2 => "updated",
						3 => "bookmarked",
						4 => "title",
						_ => "popular",
					};
					qs.push("orderby", Some(value));
					qs.push("order", Some(if ascending { "asc" } else { "desc" }));
				}
				FilterValue::Select { id, value } => {
					qs.set(&id, Some(&value));
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					if !included.is_empty() {
						qs.push(&id, Some(&format!("[\"{}\"]", included.join("\",\""))));
					}
					if !excluded.is_empty() {
						qs.push(
							&format!("{id}_exclude"),
							Some(&format!("[\"{}\"]", excluded.join("\",\""))),
						);
					}
				}
				_ => {}
			}
		}

		let url = format!(
			"{}/wp-admin/admin-ajax.php?action=advanced_search",
			params.base_url
		);
		let html = Html::parse_fragment_with_url(
			Request::post(url)?
				.body(qs.to_string())
				.header("Content-Type", "application/x-www-form-urlencoded")
				.string()?,
			&params.base_url,
		)?;

		Ok(MangaPageResult {
			entries: html
				.select("body > div")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("a.text-base")?;
						Some(Manga {
							key: link
								.attr("href")?
								.strip_prefix_or_self(&params.base_url)
								.into(),
							title: link.text()?,
							cover: el.select_first("img")?.attr("src"),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default(),
			has_next_page: html.select("body > div.flex button > svg").is_some(),
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
			manga.title = html
				.select_first("h1.text-2xl")
				.and_then(|h1| h1.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first("img.object-cover.wp-post-image")
				.and_then(|img| img.attr("src"))
				.or(manga.cover);
			manga.description = html
				.select_first("#tabpanel-description div[itemprop=description]")
				.and_then(|div| div.text());
			manga.url = Some(manga_url.clone());
			manga.tags = html
				.select("#tabpanel-description a[itemprop=genre]")
				.map(|els| els.filter_map(|el| el.text()).collect());
			let tags = manga.tags.as_deref().unwrap_or(&[]);
			manga.content_rating = if tags
				.iter()
				.any(|e| matches!(e.as_str(), "Adult" | "Hentai" | "Mature"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = html
				.select_first("div.space-y-2 > div > h4:contains(Type) + div.inline > p")
				.and_then(|span| span.text())
				.map(|text| match text.to_lowercase().as_str() {
					"manga" | "oel" | "one-shot" => Viewer::RightToLeft,
					"manhua" | "manhwa" => Viewer::Webtoon,
					_ => Viewer::Unknown,
				})
				.unwrap_or_default();

			send_partial_result(&manga);
		}

		if needs_chapters {
			let Some(manga_id) = html
				.select_first("body")
				.expect("body element must exist")
				.class_name()
				.and_then(|c| {
					c.split(" ")
						.find_map(|s| s.strip_prefix("postid-").map(|s| s.to_string()))
				})
			else {
				bail!("missing manga id");
			};
			let url = format!(
				"{}/wp-admin/admin-ajax.php?manga_id={manga_id}&page=1&action=chapter_list",
				params.base_url
			);
			let html = Html::parse_fragment_with_url(Request::get(&url)?.string()?, &manga_url)?;

			manga.chapters = html.select("#chapter-list > div").map(|els| {
				els.filter_map(|el| {
					let chapter_number = el
						.attr("data-chapter-number")
						.and_then(|s| s.parse::<f32>().ok())?;
					let a = el.select_first("a")?;
					let link = a.attr("abs:href")?;
					Some(Chapter {
						key: link.strip_prefix_or_self(&params.base_url).into(),
						chapter_number: Some(chapter_number),
						date_uploaded: el
							.select_first("time")
							.and_then(|time| time.attr("datetime"))
							.and_then(|datetime| parse_date(datetime, "yyyy-MM-dd'T'HH:mm:ss'Z'")),
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

		Ok(html
			.select("section[data-image-data] > img")
			.map(|els| {
				els.filter_map(|el| {
					Some(Page {
						content: PageContent::url(el.attr("src")?),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default())
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let url = format!("{}{}?the_page={page}", params.base_url, listing.id);
		let html = Request::get(url)?.html()?;

		Ok(MangaPageResult {
			entries: html
				.select("#search-results > div")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("a")?;
						Some(Manga {
							key: link
								.attr("href")?
								.strip_prefix_or_self(&params.base_url)
								.into(),
							title: el.select_first("h1")?.text()?,
							cover: el.select_first("img")?.attr("src"),
							..Default::default()
						})
					})
					.collect()
				})
				.unwrap_or_default(),
			has_next_page: html
				.select("div.flex.items-center.gap-2 > a > svg")
				.is_some(),
		})
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = Request::get(&params.base_url)?.html()?;

		let mut components = Vec::new();

		if let Some(hero) = html.select_first("section.hero-slider") {
			components.push(HomeComponent {
				title: None,
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: hero
						.select(".swiper > .swiper-wrapper > .swiper-slide")
						.map(|els| {
							els.filter_map(|el| {
								let link = el.select_first("a")?;
								let key = link
									.attr("href")?
									.strip_prefix_or_self(&params.base_url)
									.into();
								Some(Manga {
									key,
									title: link
										.select_first("span")
										.and_then(|h| h.text())
										.unwrap_or_default(),
									cover: el.select_first("img").and_then(|img| img.attr("src")),
									description: link.select_first("div").and_then(|el| el.text()),
									tags: el
										.select("span > a")
										.map(|els| els.filter_map(|el| el.text()).collect()),
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

		if let Some(popular_today) = html.select_first(".trending-slider") {
			components.push(HomeComponent {
				title: Some("Popular Today".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: popular_today
						.select(".swiper > .swiper-wrapper > .swiper-slide")
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
											.select_first(".title > h4")
											.and_then(|a| a.text())
											.unwrap_or_default(),
										cover: el
											.select_first("img")
											.and_then(|img| img.attr("src")),
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

		if let Some(chapter_lists) = html.select("div.project.group") {
			for list in chapter_lists {
				let title = list
					.select_first("h2")
					.and_then(|h2| h2.own_text())
					.map(|s| s.trim().into());
				components.push(HomeComponent {
					title: title.clone(),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						page_size: None,
						entries: list
							.select(".grid > div")
							.map(|els| {
								els.filter_map(|el| {
									let link = el.select_first("a")?;
									let chapter_link = el.select_first("ul > li a")?;
									let manga_key = link
										.attr("href")?
										.strip_prefix_or_self(&params.base_url)
										.into();
									let chapter_key = chapter_link
										.attr("href")?
										.strip_prefix_or_self(&params.base_url)
										.into();
									Some(MangaWithChapter {
										manga: Manga {
											key: manga_key,
											title: link.attr("title").unwrap_or_default(),
											cover: el
												.select_first("img")
												.and_then(|img| img.attr("src")),
											..Default::default()
										},
										chapter: Chapter {
											key: chapter_key,
											title: chapter_link.text(),
											..Default::default()
										},
									})
								})
								.collect()
							})
							.unwrap_or_default(),
						listing: list
							.select_first("h2 + a")
							.and_then(|el| el.attr("href"))
							.map(|href| Listing {
								id: href.strip_prefix_or_self(&params.base_url).into(),
								name: title.unwrap_or_default(),
								..Default::default()
							}),
					},
				});
			}
		}

		if let Some(ranking) = html.select_first(".widget_trending_posts") {
			let title = ranking
				.select_first("h3")
				.and_then(|h2| h2.own_text())
				.map(|s| s.trim().into());
			components.push(HomeComponent {
				title,
				subtitle: None,
				value: HomeComponentValue::MangaList {
					ranking: true,
					page_size: None,
					entries: ranking
						.select_first(".trending-content > ul")
						.and_then(|ul| {
							ul.select("li").map(|els| {
								els.filter_map(|el| {
									let link = el.select_first("h2 > a")?;
									let key = link
										.attr("href")?
										.strip_prefix_or_self(&params.base_url)
										.into();
									Some(
										Manga {
											key,
											title: link.text().unwrap_or_default(),
											cover: el
												.select_first("img")
												.and_then(|img| img.attr("src")),
											..Default::default()
										}
										.into(),
									)
								})
								.collect()
							})
						})
						.unwrap_or_default(),
					listing: None,
				},
			});
		}

		Ok(HomeLayout { components })
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url.as_ref()) else {
			return Ok(None);
		};

		const MANGA_PATH: &str = "/manga/";
		if !path.starts_with(MANGA_PATH) {
			return Ok(None);
		}

		let mut third_slash_pos = None;
		let mut slash_count = 0;
		for (i, c) in path.char_indices() {
			if c == '/' {
				slash_count += 1;
				if slash_count == 3 {
					third_slash_pos = Some(i);
				}
			}
		}

		if let Some(idx) = third_slash_pos
			&& (slash_count > 3 || (slash_count == 3 && !path.ends_with('/')))
		{
			// ex: https://rawkuma.net/manga/one-piece/chapter-999.128019/
			let manga_key = &path[..=idx];
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: path.into(),
			}))
		} else {
			// ex: https://rawkuma.net/manga/one-piece/
			Ok(Some(DeepLinkResult::Manga { key: path.into() }))
		}
	}
}
