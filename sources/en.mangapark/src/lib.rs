#![no_std]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeComponentValue, HomeLayout, Link, Listing, ListingKind, ListingProvider, Manga,
	MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::{html::*, net::*},
	prelude::*,
};

mod filter;
mod helper;
mod model;

const BASE_URL: &str = "https://mangapark.com";
const PAGE_SIZE: i32 = 18;
struct MangaPark;

impl Source for MangaPark {
	fn new() -> Self {
		Self
	}
	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = format!(
			"{BASE_URL}/search?&{}&page={}",
			filter::get_filters(query, filters),
			page
		);
		let html = Request::get(&url)?.header("Cookie", "nsfw=2").html()?;
		let entries = html
			.select("[q:key=\"q4_9\"]")
			.map(|els| {
				els.filter_map(|el| {
					let manga_url = el.select_first("a")?.attr("abs:href");
					let cover = el.select_first("img")?.attr("abs:src").unwrap_or_default();
					let manga_key: String = manga_url.as_ref()?.strip_prefix(BASE_URL)?.into();
					let title = el.select_first("[q:key=\"o2_2\"]")?.text()?;
					Some(Manga {
						key: manga_key,
						title,
						cover: Some(cover),
						url: manga_url,
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
		let html = Request::get(&manga_url)?.header("Cookie", "nsfw=2").html()?;
		if needs_details {
			let description_tag = html
				.select(".limit-html-p")
				.and_then(|desc| desc.text())
				.unwrap_or_default();
			manga.description = Some(description_tag);
			let status_str = html
				.select("[q:key=\"Yn_9\"] > span.uppercase")
				.and_then(|status| status.text())
				.unwrap_or_default();
			manga.status = match status_str.as_str() {
				"Complete" => MangaStatus::Completed,
				"Ongoing" => MangaStatus::Ongoing,
				"Hiatus" => MangaStatus::Hiatus,
				"Canceled" => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			};
			let authors_str = html
				.select("[q:key=\"tz_4\"] > a")
				.and_then(|els| els.text())
				.unwrap_or_default();
			let authors: Vec<String> = authors_str.split(" ").map(|s| s.to_string()).collect();
			manga.authors = Some(authors);
			let manga_tags: Vec<String> = html
				.select("[q:key=\"kd_0\"]")
				.map(|els| els.filter_map(|el| el.text()).collect())
				.unwrap_or_default();
			manga.tags = Some(manga_tags);
			let tags = manga.tags.as_deref().unwrap_or_default();
			manga.content_rating = if tags
				.as_ref()
				.iter()
				.any(|e| matches!(e.as_str(), "Doujinshi" | "Adult" | "Mature" | "Smut"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
			manga.viewer = if tags.as_ref().iter().any(|e| e == "Manga") {
				Viewer::RightToLeft
			} else if tags
				.iter()
				.any(|e| matches!(e.as_str(), "Manhwa" | "Manhua" | "Webtoon"))
			{
				Viewer::Webtoon
			} else {
				Viewer::Unknown
			}
		}

		if needs_chapters {
			manga.chapters = html.select("[q:key=\"8t_8\"]").map(|elements| {
				elements
					.filter_map(|element| {
						let links = element.select_first("a");
						let url = links
							.as_ref()
							.and_then(|el| el.attr("abs:href"))
							.unwrap_or_default();
						let key = url.strip_prefix(&manga_url).unwrap_or_default().into();
						let vol_and_chap_number =
							links.as_ref().and_then(|el| el.text()).unwrap_or_default();
						let title = element
							.select_first("[q:key=\"8t_1\"]")
							.and_then(|element| element.text())
							.unwrap_or_default();
						let mut volume_number: Option<f32> = None;
						let mut chapter_number: Option<f32> = None;
						let mut final_title: Option<String> = Some(vol_and_chap_number.to_string());
						let is_chapter = vol_and_chap_number.contains("Ch");
						if is_chapter {
							let (vol_num, ch_num, chapter_title) =
								helper::get_volume_and_chapter_number(vol_and_chap_number);
							volume_number = vol_num;
							chapter_number = ch_num;
							final_title = chapter_title;
						}
						if !title.is_empty() {
							final_title =
								Some(title.strip_prefix(":").unwrap_or(&title).trim().to_string());
						}
						let date_uploaded = element
							.select_first("time")
							.and_then(|el| el.attr("data-time"))?
							.parse::<i64>()
							.ok()
							.and_then(chrono::DateTime::from_timestamp_millis)
							.map(|d| d.timestamp())
							.unwrap_or_default();
						Some(Chapter {
							key,
							title: final_title,
							chapter_number,
							volume_number,
							date_uploaded: Some(date_uploaded),
							url: Some(url),
							..Default::default()
						})
					})
					.collect::<Vec<Chapter>>()
			});
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_url = format!("{BASE_URL}{}{}", manga.key, chapter.key);
		let html = Request::get(&chapter_url)?.header("Cookie", "nsfw=2").html()?;
		let mut pages: Vec<Page> = Vec::new();
		let script_str = html
			.select("[type=\"qwik/json\"]")
			.and_then(|el| el.html())
			.unwrap_or_default();
		let chap = script_str.find("\"https://s").unwrap_or(0);
		let mut end = script_str.find("whb").unwrap_or(0);
		if end == 0 {
			end = script_str.find("bwh").unwrap_or(0);
		}
		let mut text_slice = script_str[chap..end].to_string();
		text_slice = text_slice.replace("\"", "");
		text_slice.pop();
		let arr = text_slice
			.split(",")
			.map(|s| s.to_string())
			.collect::<Vec<String>>();
		for page_url in arr {
			pages.push(Page {
				content: PageContent::url(page_url),
				..Default::default()
			});
		}
		Ok(pages)
	}
}

impl ListingProvider for MangaPark {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "latest" {
			let html = Request::get(format!("{BASE_URL}/latest/{page}"))?.header("Cookie", "nsfw=2").html()?;
			let entries = html
				.select("[q:key=\"Di_7\"]")
				.map(|els| {
					els.filter_map(|el| {
						let manga_key = el
							.select_first("a")?
							.attr("abs:href")?
							.strip_prefix(BASE_URL)?
							.into();
						let cover = el.select_first("img")?.attr("abs:src").unwrap_or_default();
						let title = el.select_first("[q:key=\"o2_2\"]")?.text()?;
						Some(Manga {
							key: manga_key,
							title,
							cover: Some(cover),
							..Default::default()
						})
					})
					.collect::<Vec<Manga>>()
				})
				.unwrap_or_default();

			let mut has_next_page = true;
			if page == 99 {
				has_next_page = false;
			}
			Ok(MangaPageResult {
				entries,
				has_next_page,
			})
		} else {
			bail!("Invalid listing");
		}
	}
}

impl Home for MangaPark {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.header("Cookie", "nsfw=2").html()?;

		fn parse_manga_with_chapter_with_details(el: &Element) -> Option<MangaWithChapter> {
			let links = el.select_first("a")?;
			let manga_title = el.select("h3 span")?.text()?;
			let cover = el.select_first("img")?.attr("abs:src");
			let manga_url = links.attr("abs:href").unwrap_or_default();
			let manga_key: String = manga_url.strip_prefix(BASE_URL)?.into();
			let ch_el = el.select("[q:key=\"R7_8\"]")?;
			let ch_url = ch_el
				.select_first("a")?
				.attr("abs:href")
				.unwrap_or_default();
			let ch_title = ch_el.select_first("a > span")?.text().unwrap_or_default();
			let ch_key: String = ch_url.strip_prefix(&manga_url)?.into();
			let date_uploaded = el
				.select_first("time")
				.and_then(|el| el.attr("data-time"))?
				.parse::<i64>()
				.ok()
				.and_then(chrono::DateTime::from_timestamp_millis)
				.map(|d| d.timestamp())
				.unwrap_or_default();
			Some(MangaWithChapter {
				manga: Manga {
					key: manga_key,
					title: manga_title,
					cover,
					url: Some(manga_url),
					..Default::default()
				},
				chapter: Chapter {
					key: ch_key,
					title: Some(ch_title),
					date_uploaded: Some(date_uploaded),
					url: Some(ch_url),
					..Default::default()
				},
			})
		}

		fn parse_manga(el: &Element) -> Option<Manga> {
			let manga_url = el.select_first("a")?.attr("abs:href");
			let cover = el.select_first("img")?.attr("abs:src");
			let manga_key: String = manga_url.as_ref()?.strip_prefix(BASE_URL)?.into();
			let title = el
				.select_first("a.font-bold")
				.unwrap()
				.text()
				.unwrap_or_default();
			Some(Manga {
				key: manga_key,
				title,
				cover,
				url: manga_url,
				..Default::default()
			})
		}
		let popular_updates = html
			.select("[q:key=\"xL_7\"]")
			.map(|els| {
				els.filter_map(|el| parse_manga(&el).map(Into::into))
					.collect::<Vec<Link>>()
			})
			.unwrap_or_default();

		let member_uploads = html
			.select("[q:key=\"QJ_7\"]")
			.map(|els| {
				els.filter_map(|el| parse_manga_with_chapter_with_details(&el))
					.collect::<Vec<MangaWithChapter>>()
			})
			.unwrap_or_default();

		let latest_releases = html
			.select("[q:key=\"Di_7\"]")
			.map(|els| {
				els.filter_map(|el| parse_manga_with_chapter_with_details(&el))
					.collect::<Vec<MangaWithChapter>>()
			})
			.unwrap_or_default();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some(("Popular Releases").into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: popular_updates,
						listing: None,
					},
				},
				HomeComponent {
					title: Some(("Member Uploads").into()),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						page_size: Some(PAGE_SIZE),
						entries: member_uploads,
						listing: None,
					},
				},
				HomeComponent {
					title: Some(("Latest Releases").into()),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						page_size: Some(PAGE_SIZE),
						entries: latest_releases,
						listing: Some(Listing {
							id: "latest".into(),
							name: "Latest Releases".into(),
							..Default::default()
						}),
					},
				},
			],
		})
	}
}

impl DeepLinkHandler for MangaPark {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}
		let key = &url[BASE_URL.len()..]; // remove base url prefix
		const LATEST_PATH: &str = "/latest";
		let num_sections: usize = url.split("/").count();
		if key.starts_with(LATEST_PATH) {
			Ok(Some(DeepLinkResult::Listing(Listing {
				id: "latest".to_string(),
				name: "Latest Releases".to_string(),
				kind: ListingKind::Default,
			})))
		} else if num_sections == 2 {
			// ex: https://mangapark.com/title/408288-en-eternally-regressing-knight
			Ok(Some(DeepLinkResult::Manga { key: key.into() }))
		} else if num_sections == 3 {
			// ex: https://mangapark.com/title/408288-en-eternally-regressing-knight/9831073-chapter-73
			let split_sections: Vec<String> = key.split("/").map(|s| s.to_string()).collect();
			let manga_key: String = split_sections[0..2].join(",").to_string();
			let chapter_key: String = split_sections[3].to_string();
			Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: chapter_key,
			}))
		} else {
			Ok(None)
		}
	}
}

register_source!(MangaPark, ListingProvider, Home, DeepLinkHandler);
