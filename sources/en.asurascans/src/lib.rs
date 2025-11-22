#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeComponentValue, HomeLayout, Link, Manga, MangaPageResult, MangaStatus, MangaWithChapter,
	Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		net::{Request, TimeUnit, set_rate_limit},
		std::parse_date,
	},
	prelude::*,
};

mod helpers;

const BASE_URL: &str = "https://asuracomic.net";

struct AsuraScans;

impl Source for AsuraScans {
	fn new() -> Self {
		set_rate_limit(2, 2, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		if query.is_some() {
			qs.push("name", query.as_deref());
		}

		for filter in filters {
			match filter {
				FilterValue::Sort { id, index, .. } => {
					qs.set(
						&id,
						Some(match index {
							0 => "update",
							1 => "rating",
							2 => "bookmarks",
							3 => "desc",
							4 => "asc",
							_ => "update",
						}),
					);
				}
				FilterValue::Select { id, value } => {
					qs.push(&id, Some(&value));
				}
				FilterValue::MultiSelect { id, included, .. } => {
					qs.push(&id, Some(&included.join(",")));
				}
				_ => continue,
			}
		}

		let url = format!("{BASE_URL}/series?{qs}");
		let html = Request::get(url)?.html()?;

		let entries = html
			.select("div.grid > a[href]")
			.map(|els| {
				els.filter_map(|el| {
					Some(Manga {
						key: el
							.attr("abs:href")
							.and_then(|url| helpers::get_manga_key(&url))?,
						title: el.select_first("div.block > span.block")?.own_text()?,
						cover: el.select_first("img").and_then(|el| el.attr("abs:src")),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		let has_next_page = html
			.select_first("div.flex > a.flex.bg-themecolor:contains(Next)")
			.is_some();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = helpers::get_manga_url(&manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			manga.title = html
				.select_first("span.text-xl.font-bold, h3.truncate")
				.and_then(|el| el.own_text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first("img[alt=poster]")
				.and_then(|el| el.attr("abs:src"));
			manga.artists = html
				.select_first("div.grid > div:has(h3:eq(0):containsOwn(Artist)) > h3:eq(1)")
				.and_then(|el| el.text())
				.and_then(|s| if s != "_" { Some(vec![s]) } else { None });
			manga.authors = html
				.select_first("div.grid > div:has(h3:eq(0):containsOwn(Author)) > h3:eq(1)")
				.and_then(|el| el.text())
				.and_then(|s| if s != "_" { Some(vec![s]) } else { None });
			manga.description = html
				.select_first("span.font-medium.text-sm")
				.and_then(|el| el.text());
			manga.url = Some(url);
			manga.tags = html
				.select("div[class^=space] > div.flex > button.text-white")
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.status = html
				.select_first("div.flex:has(h3:eq(0):containsOwn(Status)) > h3:eq(1)")
				.and_then(|el| el.text())
				.map(|s| match s.as_str() {
					"Ongoing" => MangaStatus::Ongoing,
					"Hiatus" => MangaStatus::Hiatus,
					"Completed" => MangaStatus::Completed,
					"Dropped" => MangaStatus::Cancelled,
					"Season End" => MangaStatus::Hiatus,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or_default();
			let tags = manga.tags.as_deref().unwrap_or_default();
			manga.content_rating = if tags
				.as_ref()
				.iter()
				.any(|e| matches!(e.as_str(), "Adult" | "Ecchi"))
			{
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = html
				.select_first("div.flex:has(h3:eq(0):containsOwn(Type)) > h3:eq(1)")
				.and_then(|el| el.text())
				.map(|s| match s.as_str() {
					"Manhwa" => Viewer::Webtoon,
					"Manhua" => Viewer::Webtoon,
					"Manga" => Viewer::RightToLeft,
					_ => Viewer::Webtoon,
				})
				.unwrap_or(Viewer::Webtoon);
		}

		if needs_chapters {
			manga.chapters = html
				.select("div.scrollbar-thumb-themecolor > div.group")
				.map(|els| {
					els.filter_map(|el| {
						let raw_url = el.select_first("a")?.attr("abs:href")?;
						let key = helpers::get_chapter_key(&raw_url)?;
						let title = el.select("h3 > span").and_then(|els| els.text());
						let chapter_number = el
							.select_first("h3.text-sm")
							.and_then(|el| el.own_text())
							.and_then(|s| s.trim_start_matches("Chapter ").parse().ok());
						let date_uploaded = el
							.select_first("h3 + h3")
							.and_then(|els| els.own_text())
							.map(|s| {
								let mut parts = s.split_whitespace().collect::<Vec<&str>>();

								// Check if the date has 3 parts, Month Day Year
								if parts.len() == 3 {
									let day = parts[1];

									// Remove any non-digit characters from the day
									// We are trying to remove all the suffixes from the day
									let cleaned_day = day
										.chars()
										.filter(|c| c.is_ascii_digit())
										.collect::<String>();

									parts[1] = &cleaned_day;

									parts.join(" ")
								} else {
									s
								}
							})
							.and_then(|s| parse_date(s, "MMMM d yyyy"));
						let url = helpers::get_chapter_url(&key, &manga.key);
						let locked = el.select_first("h3 > span > svg").is_some();
						Some(Chapter {
							key,
							title,
							chapter_number,
							date_uploaded,
							url: Some(url),
							locked,
							..Default::default()
						})
					})
					.collect()
				})
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = helpers::get_chapter_url(&chapter.key, &manga.key);
		let response = Request::get(url)?.string()?;

		// Remove script tags from hydration that can cut up the page list
		let html_text = response.replace(r#""])</script><script>self.__next_f.push([1,""#, "");

		// Find bounds of the page list JSON
		let page_list_marker = r#"\"pages\":[{\"order\":1,\"url\":\"https://"#;
		let page_list_start = html_text.find(page_list_marker).unwrap_or(0);
		let page_list_end = html_text[page_list_start..].find(r#"}]"#).unwrap_or(0);

		let page_list_slice = &html_text[page_list_start..page_list_start + page_list_end];

		let mut pages = Vec::new();
		let mut search_start = 0;

		while let Some(pos) =
			page_list_slice[search_start..].find("https://gg.asuracomic.net/storage/media/")
		{
			let url_start = search_start + pos;
			let rest = &page_list_slice[url_start..];
			if let Some(url_end) = rest.find('"') {
				let url = rest[..url_end].replace("\\", "");
				pages.push(Page {
					content: PageContent::url(url),
					..Default::default()
				});
				search_start = url_start + url_end;
			} else {
				break;
			}
		}

		Ok(pages)
	}
}

impl Home for AsuraScans {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;

		let mut components = Vec::new();

		if let Some(hero) = html.select_first("div.owl-carousel") {
			let entries: Vec<Manga> = hero
				.select(".slider > .slide")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("a")?;
						let key = helpers::get_manga_key(&link.attr("abs:href")?)?;
						Some(Manga {
							key,
							title: link.text()?,
							cover: el.select_first("img").and_then(|img| img.attr("abs:src")),
							description: el.select_first("div.summary").and_then(|el| el.text()),
							tags: el
								.select("span.extra-category:not(.hidden) > a")
								.map(|els| {
									els.filter_map(|el| el.text())
										.map(|s| s.trim_end_matches(",").into())
										.filter(|s: &String| !s.is_empty() && s != "...")
										.collect()
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

		if let Some(popular_today) = html.select_first("div.text-white.pt-2") {
			let title = popular_today
				.select_first("h3")
				.and_then(|el| el.text())
				.unwrap_or("Popular Today".into());
			let entries: Vec<Link> = popular_today
				.select("div.flex-wrap.hidden > div > a")
				.map(|els| {
					els.filter_map(|el| {
						let key = helpers::get_manga_key(&el.attr("abs:href")?)?;
						Some(
							Manga {
								key,
								title: el.select_first("span.block")?.text()?,
								cover: el.select_first("img").and_then(|img| img.attr("abs:src")),
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

		if let Some(latest_updates) = html.select_first("div.text-white.mb-1") {
			let title = latest_updates
				.select_first("h3")
				.and_then(|el| el.text())
				.unwrap_or("Latest Updates".into());
			let entries: Vec<MangaWithChapter> = latest_updates
				.select(".grid > div > .grid")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("span > a")?;
						let chapter_link = el.select_first(".flex > span a")?;
						let manga_key = helpers::get_manga_key(&link.attr("abs:href")?)?;
						let chapter_key =
							helpers::get_chapter_key(&chapter_link.attr("abs:href")?)?;
						let chapter_number = chapter_link
							.select_first("p")?
							.text()?
							.strip_prefix("Chapter ")?
							.split(" ")
							.next()?
							.parse()
							.ok();
						Some(MangaWithChapter {
							manga: Manga {
								key: manga_key,
								title: link.text()?,
								cover: el.select_first("img").and_then(|img| img.attr("abs:src")),
								..Default::default()
							},
							chapter: Chapter {
								key: chapter_key,
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
			}
		}

		Ok(HomeLayout { components })
	}
}

impl DeepLinkHandler for AsuraScans {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(manga_key) = helpers::get_manga_key(&url) else {
			return Ok(None);
		};

		if let Some(chapter_key) = helpers::get_chapter_key(&url) {
			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: chapter_key,
			}))
		} else {
			Ok(Some(DeepLinkResult::Manga { key: manga_key }))
		}
	}
}

register_source!(AsuraScans, Home, DeepLinkHandler);
