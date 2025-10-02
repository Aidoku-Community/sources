#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeLayout, Link,
	Manga, MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent, Result, Source,
	Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::{html::*, net::*},
	prelude::*,
};
use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};

mod filter;

const BASE_URL: &str = "https://flamecomics.xyz";

struct FlameComics;

impl Source for FlameComics {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let query = query.unwrap_or_default().to_ascii_lowercase();
		let genre = filter::get_genre_filter(&filters);
		let url = if !genre.is_empty() {
			format!("{BASE_URL}/{genre}")
		} else {
			format!("{BASE_URL}/browse")
		};
		let html = Request::get(url)?.html()?;
		let entries = html
			.select(".mantine-Container-root .mantine-Grid-root:nth-of-type(2) .mantine-Grid-col .mantine-Group-root")
			.map(|els| {
				els.filter_map(|el| {
					let manga_url = el.select_first("a")?.attr("abs:href").unwrap_or_default();
					let cover = el.select_first("img")?.attr("abs:src").unwrap_or_default();
					let manga_key = manga_url.strip_prefix(BASE_URL)?.into();
					let title = el.select_first(".mantine-Stack-root a")?.text()?;
					Some(Manga {
						key: manga_key,
						title,
						cover: Some(cover),
						url: Some(manga_url),
						..Default::default()
					})
				})
				.collect::<Vec<Manga>>()
			})
			.unwrap_or_default();
		// Filtering out the search query
		let mut entries = if query.is_empty() {
			entries
		} else {
			entries
				.into_iter()
				.filter(|manga| manga.title.to_lowercase().contains(&query))
				.collect()
		};
		entries = filter::sort(&filters, entries);
		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&manga_url)?.html()?;

		if needs_details {
			const MANGA_AUTHORS: &str = ".mantine-Grid-root .mantine-Grid-col:nth-of-type(1) .mantine-Stack-root .mantine-Stack-root";
			const MANGA_DETAILS: &str = ".mantine-Grid-root .mantine-Grid-col:nth-of-type(2) .mantine-Stack-root .mantine-Paper-root:nth-of-type(1)";
			let details = html
				.select_first(MANGA_DETAILS)
				.ok_or(error!("Missing manga details"))?;
			let authors = html
				.select_first(MANGA_AUTHORS)
				.ok_or(error!("Missing manga authors"))?;
			let description = details
				.select("p")
				.and_then(|desc| desc.text())
				.unwrap_or_default();
			// Extract text between <p> and </p> tags of the description...
			let description = Html::parse_fragment(&description)?
				.select("p")
				.map(|els| {
					els.filter_map(|el| el.text())
						.collect::<Vec<String>>()
						.join("\n")
				})
				.unwrap_or_default();
			manga.description = Some(description);
			let status_str = details
				.select(".mantine-Badge-root")
				.and_then(|el| el.select_first("span"))
				.and_then(|status| status.text())
				.unwrap_or_default();
			manga.status = match status_str.as_str() {
				"Complete" => MangaStatus::Completed,
				"Ongoing" => MangaStatus::Ongoing,
				"Hiatus" => MangaStatus::Hiatus,
				"Canceled" | "Dropped" => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			};
			let manga_tags: Vec<String> = details
				.select(".mantine-Group-root:last-of-type .mantine-Badge-label")
				.map(|els| els.filter_map(|el| el.text()).collect())
				.unwrap_or_default();
			manga.tags = Some(manga_tags);
			let manga_artist = authors
				.select(".mantine-Paper-root:nth-of-type(1) p:last-of-type")
				.and_then(|el| el.text())
				.unwrap_or_default()
				.split(",")
				.map(|s| s.to_string())
				.collect::<Vec<String>>();
			manga.artists = Some(manga_artist);
			let manga_author = authors
				.select(".mantine-Paper-root:nth-of-type(2) p:last-of-type")
				.and_then(|el| el.text())
				.unwrap_or_default()
				.split(",")
				.map(|s| s.to_string())
				.collect::<Vec<String>>();
			manga.authors = Some(manga_author);
			manga.viewer = Viewer::Webtoon;
		}

		if needs_chapters {
			const CHAPTER_SELECTOR: &str = ".mantine-Grid-root .mantine-Grid-col:nth-of-type(2) .mantine-Stack-root .mantine-Paper-root:nth-of-type(2) .mantine-ScrollArea-viewport a";
			manga.chapters = html.select(CHAPTER_SELECTOR).map(|elements| {
				elements
					.map(|element| {
						let url = element.attr("abs:href").unwrap_or_default();
						let key: String = url.strip_prefix(&manga_url).unwrap_or_default().into();
						let title: String = element
							.select_first(".mantine-Stack-root p")
							.and_then(|e| e.text())
							.unwrap_or_default();
						let chapter_number: Option<f32> = title
							.split(" ")
							.nth(1)
							.and_then(|num| num.parse::<f32>().ok());
						let date_uploaded: String = element
							.select_first(".mantine-Stack-root p:last-of-type")
							.and_then(|e| e.attr("title"))
							.unwrap_or_default();
						let format: &str = "%B %e, %Y %l:%M %p";
						let naive_date = NaiveDateTime::parse_from_str(&date_uploaded, format)
							.expect("Error: Expected date string");
						let offset = FixedOffset::west_opt(7 * 3600).unwrap();
						let dt_with_tz: DateTime<FixedOffset> =
							offset.from_local_datetime(&naive_date).unwrap();
						let utc_dt: DateTime<Utc> = dt_with_tz.with_timezone(&Utc);
						let chapter_date = utc_dt.timestamp();
						Chapter {
							key,
							chapter_number,
							date_uploaded: Some(chapter_date),
							url: Some(url),
							..Default::default()
						}
					})
					.collect::<Vec<Chapter>>()
			});
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_url = format!("{BASE_URL}{}{}", manga.key, chapter.key);
		let html = Request::get(&chapter_url)?.html()?;
		let pages = html
			.select(".mantine-Container-root .mantine-Stack-root img")
			.map(|els| {
				els.filter_map(|el| {
					let page_url = el.attr("abs:src")?;
					Some(Page {
						content: PageContent::url(page_url),
						..Default::default()
					})
				})
				.collect::<Vec<Page>>()
			})
			.unwrap_or_default();
		Ok(pages)
	}
}

impl Home for FlameComics {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;
		fn parse_manga_with_chapter(el: &Element) -> Option<MangaWithChapter> {
			let manga_url = el.select_first("a")?.attr("abs:href").unwrap_or_default();
			let cover = el.select_first("img")?.attr("abs:src");
			let manga_key: String = manga_url.strip_prefix(BASE_URL)?.into();
			let title = el
				.select_first(".mantine-Stack-root:nth-of-type(2) a")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let chapter_details: ElementList =
				el.select(".mantine-Text-root:nth-of-type(1)").unwrap();
			let binding = chapter_details
				.select_first("p")
				.and_then(|t| t.text())
				.unwrap_or_default();
			let chapter_number = binding
				.split(' ')
				.next_back()
				.and_then(|num| num.parse::<f32>().ok());
			Some(MangaWithChapter {
				manga: Manga {
					key: manga_key,
					title,
					cover,
					..Default::default()
				},
				chapter: Chapter {
					chapter_number,
					..Default::default()
				},
			})
		}

		fn parse_manga(el: &Element) -> Option<Manga> {
			let manga_url = el.select_first("a")?.attr("abs:href").unwrap_or_default();
			let cover = el.select_first("img")?.attr("abs:src");
			let manga_key: String = manga_url.strip_prefix(BASE_URL)?.into();
			let title = el.select_first("p").unwrap().text().unwrap_or_default();
			Some(Manga {
				key: manga_key,
				title,
				cover,
				url: Some(manga_url),
				..Default::default()
			})
		}

		let popular = html
			.select(".mantine-Container-root .mantine-Grid-root:nth-of-type(1) .mantine-Grid-inner .mantine-Grid-col") // 1st child // 
			.map(|els| {
				els.filter_map(|el| parse_manga(&el).map(Into::into))
					.collect::<Vec<Link>>()
			})
			.unwrap_or_default();

		let staff_picks = html
			.select(".mantine-Container-root .mantine-Grid-root:nth-of-type(2) .mantine-Grid-inner .mantine-Grid-col")
			.map(|els| {
				els.filter_map(|el| parse_manga(&el).map(Into::into))
					.collect::<Vec<Link>>()
			})
			.unwrap_or_default();

		let latest = html
		.select(".mantine-Container-root .mantine-Grid-root:nth-of-type(3) .mantine-Grid-inner .mantine-Grid-col")
		.map(|els| {
			els.filter_map(|el| parse_manga_with_chapter(&el))
				.collect::<Vec<MangaWithChapter>>()
		})
		.unwrap_or_default();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Popular".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: popular,
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Staff Picks".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: staff_picks,
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Latest".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList {
						page_size: Some(13),
						entries: latest,
						listing: None,
					},
				},
			],
		})
	}
}

impl DeepLinkHandler for FlameComics {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}
		let key = &url[BASE_URL.len()..]; // remove base url prefix
		let num_sections: usize = url.split("/").count();
		if num_sections == 2 {
			// ex: https://flamecomics.xyz/series/2
			Ok(Some(DeepLinkResult::Manga { key: key.into() }))
		} else if num_sections == 3 {
			// ex: https://flamecomics.xyz/series/2/79c2cf38ecc5fd25
			let mut chapter_key = key.rsplit("/").next().unwrap_or_default().to_string(); // 79c2cf38ecc5fd25
			chapter_key = format!("/{chapter_key}"); // add leading slash
			let manga_key = key.strip_suffix(&chapter_key).unwrap_or_default().into();
			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: chapter_key,
			}))
		} else {
			Ok(None)
		}
	}
}

register_source!(FlameComics, Home, DeepLinkHandler);
