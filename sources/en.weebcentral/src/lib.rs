#![no_std]
use aidoku::{
	alloc::{borrow::ToOwned, vec, String, Vec},
	imports::{html::Element, net::Request, std::send_partial_result},
	prelude::*,
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeComponent, HomeLayout, ImageRequestProvider, Listing, Manga, MangaPageResult, MangaStatus,
	MangaWithChapter, Page, PageContent, Result, Source, Viewer,
};

mod filter;
mod helper;
mod model;

const BASE_URL: &str = "https://weebcentral.com";
const REFERER: &str = "https://weebcentral.com/";
const FETCH_LIMIT: i32 = 24;

struct WeebCentral;

impl Source for WeebCentral {
	fn new() -> Self {
		Self
	}

	fn get_manga_list(&self, listing: Listing, _page: i32) -> Result<MangaPageResult> {
		if listing.id == "hot" {
			let html = Request::get(format!("{BASE_URL}/hot-updates"))?.html()?;

			let entries = html
				.select("article:not(.hidden)")
				.map(|els| {
					els.filter_map(|el| {
						let manga_key = el
							.select_first("a")?
							.attr("href")?
							.strip_prefix(BASE_URL)?
							.into();
						let cover = el.select_first("img")?.attr("src");
						let title = el.select_first(".text-lg")?.text()?;
						Some(Manga {
							key: manga_key,
							title,
							cover,
							..Default::default()
						})
					})
					.collect::<Vec<_>>()
				})
				.unwrap_or_default();

			Ok(MangaPageResult {
				entries,
				has_next_page: false,
			})
		} else {
			bail!("Invalid listing");
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let offset = (page - 1) * FETCH_LIMIT;

		let url = format!(
			"{BASE_URL}/search/data\
					?limit={FETCH_LIMIT}\
					&offset={offset}\
					&display_mode=Full+Display\
					&{}",
			filter::get_filters(query, filters)
		);

		let html = Request::get(&url)?.html()?;

		let entries = html
			.select("article:has(section)")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let cover = element.select_first("img")?.attr("abs:src");

						let title_element = element.select_first("a")?;
						let mut title = title_element.text().unwrap_or_default();

						const OFFICIAL_PREFIX: &str = "Official ";
						if title.starts_with(OFFICIAL_PREFIX) {
							title = title[OFFICIAL_PREFIX.len()..].trim().into();
						}

						let url = title_element.attr("abs:href")?;
						let key = url.strip_prefix(BASE_URL).map(String::from)?;

						Some(Manga {
							key,
							title,
							cover,
							..Default::default()
						})
					})
					.collect::<Vec<Manga>>()
			})
			.unwrap_or_default();

		let has_next_page = !entries.is_empty();

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
		let manga_url = format!("{BASE_URL}{}", manga.key);

		if needs_details {
			let html = Request::get(&manga_url)?.html()?;

			let elements = html.select("section[x-data] > section");
			let (info_element, title_element) = match elements.as_ref() {
				Some(els) if !els.is_empty() => {
					let info = els.first();
					let title = els.last();
					match (info, title) {
						(Some(info), Some(title)) => (info, title),
						_ => return Err(AidokuError::Unimplemented),
					}
				}
				_ => return Err(AidokuError::Unimplemented),
			};

			let get_text = |el: &Element, sel: &str| el.select_first(sel).and_then(|e| e.text());

			manga.title = get_text(&title_element, "h1").unwrap_or(manga.title);
			manga.cover = info_element
				.select_first("img")
				.and_then(|el| el.attr("abs:src"));
			manga.authors = info_element
				.select("ul > li:has(strong:contains(Author)) > span > a")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<String>>());
			manga.description =
				get_text(&title_element, "li:has(strong:contains(Description)) > p");
			manga.url = Some(manga_url.clone());
			manga.tags = info_element
				.select("ul > li:has(strong:contains(Tag),strong:contains(Type)) a")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<String>>());

			let status_str = info_element
				.select_first("ul > li:has(strong:contains(Status)) a")
				.and_then(|x| x.text())
				.unwrap_or_default();

			manga.status = match status_str.as_str() {
				"Complete" => MangaStatus::Completed,
				"Ongoing" => MangaStatus::Ongoing,
				"Hiatus" => MangaStatus::Hiatus,
				"Canceled" => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			};

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

			let type_str =
				get_text(&info_element, "ul > li:has(strong:contains(Type)) a").unwrap_or_default();
			manga.viewer = match type_str.as_str() {
				"Manhua" | "Manhwa" => Viewer::Webtoon,
				_ => Viewer::RightToLeft,
			};

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let url = manga_url
				.rfind('/')
				.map(|pos| format!("{}/full-chapter-list", &manga_url[..pos]))
				.unwrap_or_else(|| manga_url.clone());

			let html = Request::get(&url)?.html()?;

			manga.chapters = html.select("div[x-data]").map(|elements| {
				elements
					.filter_map(|element| {
						let url = element
							.select_first("a")
							.and_then(|el| el.attr("abs:href"))?;

						let key = url.strip_prefix(BASE_URL)?.into();

						let title = element
							.select_first("span.flex > span")
							.and_then(|el| el.text());

						let mut chapter_number = title
							.as_ref()
							.and_then(|t| t.rsplit(' ').next())
							.and_then(|num| num.parse::<f32>().ok());

						let is_volume = title.as_ref().is_some_and(|t| t.contains("Volume"));
						let is_chapter = title.as_ref().is_some_and(|t| t.contains("Chapter"));

						let (final_title, volume_number) = match (is_volume, is_chapter) {
							(true, _) => (None, chapter_number.take()),
							(_, true) => (None, None),
							_ => (title, None),
						};

						let date_uploaded = element
							.select_first("time[datetime]")
							.and_then(|el| el.attr("datetime"))
							.and_then(|dt| chrono::DateTime::parse_from_rfc3339(&dt).ok())
							.map(|d| d.timestamp());

						Some(Chapter {
							key,
							title: final_title,
							chapter_number,
							volume_number,
							date_uploaded,
							url: Some(url),
							..Default::default()
						})
					})
					.collect::<Vec<_>>()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{BASE_URL}{}/images?is_prev=False&reading_style=long_strip",
			chapter.key
		);
		let html = Request::get(url)?.html()?;

		let pages = html
			.select("section[x-data~=scroll] > img")
			.map(|els| {
				els.filter_map(|el| {
					let page_url = el.attr("abs:src")?;
					Some(Page {
						content: PageContent::url(page_url),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		Ok(pages)
	}
}

impl Home for WeebCentral {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;

		fn parse_manga_with_chapter(el: &Element) -> Option<MangaWithChapter> {
			let mut links = el.select("a")?;
			let manga_key = links.first()?.attr("href")?.strip_prefix(BASE_URL)?.into();
			let chapter_link = links.next_back()?;
			let chapter_key = chapter_link.attr("href")?.strip_prefix(BASE_URL)?.into();
			let cover = el.select_first("img")?.attr("src");
			let title = el.select_first(".text-lg")?.text()?;
			let chapter_number = chapter_link
				.select_first("div.flex")?
				.text()
				.as_ref()
				.and_then(|t| t.rsplit(' ').next())
				.and_then(|num| num.parse::<f32>().ok());
			let date_uploaded = el
				.select_first("time[datetime]")
				.and_then(|el| el.attr("datetime"))
				.and_then(|dt| chrono::DateTime::parse_from_rfc3339(&dt).ok())
				.map(|d| d.timestamp());
			Some(MangaWithChapter {
				manga: Manga {
					key: manga_key,
					title,
					cover,
					..Default::default()
				},
				chapter: Chapter {
					key: chapter_key,
					chapter_number,
					date_uploaded,
					..Default::default()
				},
			})
		}

		fn parse_manga(el: &Element) -> Option<Manga> {
			let key = el
				.select_first("a")?
				.attr("href")?
				.strip_prefix(BASE_URL)?
				.into();
			let cover = el.select_first("img")?.attr("src");
			let title = el.select_first(".text-lg")?.text()?;
			Some(Manga {
				key,
				title,
				cover,
				..Default::default()
			})
		}

		let hot_updates = html
			.select("section:has(h2:contains(Hot Updates)) article:not(.hidden)")
			.map(|els| {
				els.filter_map(|el| parse_manga_with_chapter(&el))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		let latest_updates = html
			.select("section:has(h2:contains(Latest Updates)) article")
			.map(|els| {
				els.filter_map(|el| parse_manga_with_chapter(&el))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		let recommendations = html
			.select("section:has(h2:contains(Recommendations)) li.glide__slide:not(.glide__slide--clone)")
			.map(|els| els.filter_map(|el| parse_manga(&el).map(Into::into)).collect::<Vec<_>>())
			.unwrap_or_default();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Hot Updates".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList {
						page_size: Some(6),
						entries: hot_updates,
						listing: Some(Listing {
							id: "hot".into(),
							name: "Hot Updates".into(),
							..Default::default()
						}),
					},
				},
				HomeComponent {
					title: Some("Latest Updates".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList {
						page_size: Some(16),
						entries: latest_updates,
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Recommendations".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: recommendations,
						listing: None,
					},
				},
			],
		})
	}
}

impl ImageRequestProvider for WeebCentral {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", REFERER))
	}
}

impl DeepLinkHandler for WeebCentral {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}

		let key = &url[BASE_URL.len()..]; // remove base url prefix

		const SERIES_PATH: &str = "/series";
		const CHAPTER_PATH: &str = "/chapters";

		if key.starts_with(SERIES_PATH) {
			// ex: https://weebcentral.com/series/01J76XYEZYBE7Y3MEY7AEQ8MQN/Solo-Max-Level-Newbie
			Ok(Some(DeepLinkResult::Manga { key: key.into() }))
		} else if key.starts_with(CHAPTER_PATH) {
			// ex: https://weebcentral.com/chapters/01JXNANGY619TDR9F4FST2M5E8
			let html = Request::get(&url)?.html()?;
			let manga_key = html
				.select_first("main a")
				.and_then(|e| e.attr("href"))
				.and_then(|url| url.strip_prefix(BASE_URL).map(|s| s.to_owned()))
				.ok_or(AidokuError::message("Missing manga key"))?;

			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: key.into(),
			}))
		} else {
			Ok(None)
		}
	}
}

register_source!(WeebCentral, Home, DeepLinkHandler);
