#![no_std]
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeComponent, HomeComponentValue, HomeLayout, Link, Manga, MangaPageResult, MangaStatus,
	MangaWithChapter, MigrationHandler, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		defaults::defaults_get,
		net::{Request, TimeUnit, set_rate_limit},
		std::parse_date,
	},
	prelude::*,
};

mod helpers;

const BASE_URL: &str = "https://asurascans.com";

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
			qs.push("q", query.as_deref());
		}

		for filter in filters {
			match filter {
				FilterValue::Sort {
					id,
					index,
					ascending,
				} => {
					qs.push(
						&id,
						Some(match index {
							0 => "update",
							1 => "popular",
							2 => "rating",
							3 => "name",
							4 => "newest",
							_ => "update",
						}),
					);
					if ascending {
						qs.push("order", Some("asc"));
					}
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

		let url = format!("{BASE_URL}/browse?{qs}");
		let html = Request::get(url)?.html()?;

		let entries = html
			.select("#series-grid > .series-card")
			.map(|els| {
				els.filter_map(|el| {
					Some(Manga {
						key: el
							.select_first("a")?
							.attr("abs:href")
							.and_then(|url| helpers::get_manga_key(&url))?,
						title: el.select_first("h3")?.own_text()?,
						cover: el.select_first("img").and_then(|el| el.attr("abs:src")),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default();

		let has_next_page = html
			.select_first("a:contains(Next page), div.flex > a.flex.bg-themecolor:contains(Next)")
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
				.select_first("h1.text-xl.font-semibold")
				.and_then(|el| el.own_text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first("div#desktop-cover-container img")
				.and_then(|el| el.attr("abs:src"));
			manga.artists = html.select("a[href^=/browse?artist]").map(|els| {
				els.filter_map(|el| el.text())
					.filter(|s| s != "_")
					.collect()
			});
			manga.authors = html.select("a[href^=/browse?author]").map(|els| {
				els.filter_map(|el| el.text())
					.filter(|s| s != "_")
					.collect()
			});
			manga.description = html
				.select_first("div#description-text")
				.and_then(|el| el.text());
			manga.url = Some(url);
			manga.tags = html
				.select("a[href^=/browse?genres=]")
				.map(|els| els.filter_map(|el| el.text()).collect());
			manga.status = html
				.select_first(
					"div.flex.gap-3.pt-4.border-t > div:nth-child(2) > div > span.text-base",
				)
				.and_then(|el| el.text())
				.map(|s| match s.as_str() {
					"ongoing" => MangaStatus::Ongoing,
					"hiatus" => MangaStatus::Hiatus,
					"completed" => MangaStatus::Completed,
					"dropped" => MangaStatus::Cancelled,
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
				.select_first(
					"div.flex.gap-3.pt-4.border-t > div:nth-child(2) > div > span.text-base",
				)
				.and_then(|el| el.text())
				.map(|s| match s.as_str() {
					"manhwa" => Viewer::Webtoon,
					"manhua" => Viewer::Webtoon,
					"mangatoon" => Viewer::RightToLeft,
					_ => Viewer::Webtoon,
				})
				.unwrap_or(Viewer::Webtoon);
		}

		if needs_chapters {
			let island_props = html
				.select_first(
					"astro-island[component-url*=ChapterListReact], astro-island[opts*=ChapterListReact]",
				)
				.and_then(|el| el.attr("props"))
				.ok_or_else(|| error!("Missing astro-island"))?;

			let json = serde_json::from_str::<serde_json::Value>(&island_props)
				.map_err(AidokuError::JsonParseError)?;
			let chapters_arr = json["chapters"][1]
				.as_array()
				.ok_or_else(|| error!("Missing chapters"))?;

			let skip_locked = !defaults_get::<bool>("showLocked").unwrap_or(true);

			manga.chapters = Some(
				chapters_arr
					.iter()
					.filter_map(|obj| {
						let obj = obj[1].as_object()?;

						let locked = obj["is_locked"][1].as_bool().unwrap_or_default();
						if skip_locked && locked {
							return None;
						}

						let chapter_number = obj["number"][1].as_f64().map(|f| f as f32)?;
						let key = chapter_number.to_string();
						const DATE_FORMAT: &str = "yyyy-MM-dd'T'HH:mm:ss'Z'";
						let date_uploaded = obj["published_at"][1].as_str().and_then(|s| {
							if let Some((before_dot, _)) = s.split_once('.') {
								parse_date(format!("{before_dot}Z"), DATE_FORMAT)
							} else {
								parse_date(s, DATE_FORMAT)
							}
						});
						let url = helpers::get_chapter_url(&key, &manga.key);

						Some(Chapter {
							key,
							chapter_number: Some(chapter_number),
							date_uploaded,
							url: Some(url),
							locked,
							..Default::default()
						})
					})
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = helpers::get_chapter_url(&chapter.key, &manga.key);
		let html = Request::get(url)?.html()?;

		let island_props = html
			.select_first(
				"astro-island[component-url*=ChapterReader], astro-island[opts*=ChapterReader]",
			)
			.and_then(|el| el.attr("props"))
			.ok_or_else(|| error!("Missing astro-island"))?;

		let json = serde_json::from_str::<serde_json::Value>(&island_props)
			.map_err(AidokuError::JsonParseError)?;

		let page_arr = json["pages"][1]
			.as_array()
			.ok_or_else(|| error!("Missing pages"))?;

		Ok(page_arr
			.iter()
			.filter_map(|obj| {
				let url = obj[1]["url"][1].as_str()?;
				Some(Page {
					content: PageContent::url(url),
					..Default::default()
				})
			})
			.collect())
	}
}

impl Home for AsuraScans {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;

		let mut components = Vec::new();

		if let Some(trending_today) =
			html.select_first("astro-island[opts*=TrendingSection] > section")
		{
			let title = trending_today
				.select_first("h2")
				.and_then(|el| el.text())
				.unwrap_or("Trending Today".into());
			let entries: Vec<Link> = trending_today
				.select("div.embla-trending > div > div > a")
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

		if let Some(latest_updates) =
			html.select_first("astro-island[opts*=LatestUpdates] > section")
		{
			let title = latest_updates
				.select_first("h2")
				.and_then(|el| el.text())
				.unwrap_or("Latest Updates".into());
			let entries: Vec<MangaWithChapter> = latest_updates
				.select("div.grid > div.grid")
				.map(|els| {
					els.filter_map(|el| {
						let link = el.select_first("a.font-bold")?;
						let chapter_link = el.select_first("div.col-span-8 > div.flex > a")?;
						let manga_key = helpers::get_manga_key(&link.attr("abs:href")?)?;
						let chapter_key =
							helpers::get_chapter_key(&chapter_link.attr("abs:href")?)?;
						let chapter_number = chapter_link
							.select_first("span.font-medium")?
							.text()?
							.strip_prefix("Chapter")?
							.trim()
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

impl MigrationHandler for AsuraScans {
	fn handle_manga_migration(&self, key: String) -> Result<String> {
		// v12: asuracomic.net -> asurascans.com, trailing '-' removed from ids
		Ok(key.strip_suffix("-").map(Into::into).unwrap_or(key))
	}

	fn handle_chapter_migration(&self, _manga_key: String, chapter_key: String) -> Result<String> {
		Ok(chapter_key) // no change
	}
}

register_source!(AsuraScans, Home, DeepLinkHandler, MigrationHandler);
