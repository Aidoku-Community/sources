#![no_std]
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	imports::{html::*, net::*, std::print}, 
	prelude::*, 
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Page, PageContent, Result, Source
};

// mod model;
// mod filter;
/** Build w/ aidoku pkg
 * 
 * 
 */

const BASE_URL: &str = "https://mangapark.com";

const PAGE_SIZE: i32 = 20;

struct MangaPark;

impl Source for MangaPark {
	fn new() -> Self {
		Self
	}

	/* Quite challenging to select the tag with all the articles
	
	 */

	 //TODO: Filter options in filter.rs

	// this method will be called first without a query when the search page is opened,
	// then when a search query is entered or filters are changed
	fn get_search_manga_list(
		&self,
		_query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> { // Option<asdsadsa> // as.ref() 
		// URL with search/filter options to get list of manga... https://mangapark.com/search?
		let url = format!("{}/search", BASE_URL);

		// List of Manga in Selector: q:key="jp_1"
		// div[q\:key="jp_1"]

		const LIST_SELECTOR: &str = r#"div[q\:key="jp_1"]"#;
		// const MANGA_SELECTOR: &str = r#"div[q\:key="q4_9"]"#;

		let url = format!("https://mangapark.com/search?page={}", page);
		let html = Request::get(&url)?.html()?;

		let entries = html
		.select(LIST_SELECTOR)
		.map(|items| {
			items
				.filter_map(|item| {
					let div = item.select_first("div")?.text()?;
					println!("Div: {}", div);
					aidoku::prelude::println!("Div: {}", div);
					// let url = item.select_first("a")?.attr("href");
					// let title = item.select_first(".post-title")?.text()?;
					// let cover = item
					// 	.select_first("img")
					// 	.and_then(|img| img.attr("abs:src").or_else(|| img.attr("data-cfsrc")));
					Some(Manga {
						// key: url.clone()?.strip_prefix(BASE_URL)?.into(),
						// title,
						// cover,
						// url,
						..Default::default()
					})
				})
				.collect::<Vec<Manga>>()
		})
		.unwrap_or_default();		
		// aidoku::prelude::println!("HTML: {}", html.select_first(MANGA_SELECTOR).);
		// aidoku::println!("First few chars: {}", &html.read()[..100.min(html.read().len())]);
		Ok(MangaPageResult {
			entries: Vec::new(),
			has_next_page: false,
		})
		// let html = Request::get(&url)?.html()?; 

		// let entries = html
		// 	.select(LIST_SELECTOR)
		// 	.map(|elements| {
		// 		elements.filter_map(|element| {
		// 			let cover = element.select_first("img")?.attr("abs:src");

		// 			let title_element = element.select_first("a")?;
		// 			let mut title = title_element.text().unwrap_or_default();
		// 			// let mut title = element.select_first("span")?;

		// 			const OFFICIAL_PREFIX: &str = "Official ";
		// 			if title.starts_with(OFFICIAL_PREFIX) {
		// 				title = title[OFFICIAL_PREFIX.len()..].trim().into();
		// 			}

		// 			let url = title_element.attr("abs:href")?;
		// 			let key = url.strip_prefix(BASE_URL).map(String::from)?;
		// 			println!("{}", title);
		// 			// println!("{}", )
		// 			Some(Manga {
		// 				key,
		// 				title,
		// 				cover,
		// 				..Default::default()
		// 			})
		// 		})
		// 	} );

			// let mut entries: Vec<Manga> = Vec::new();
			// let start = (page - 1) * PAGE_SIZE + 1;
			// for i in start..start + PAGE_SIZE {
			// 	let title = format!("Manga {i}");
			// 	if let Some(query) = query.as_ref() {
			// 		if !title.contains(query) {
			// 			continue;
			// 		}
			// 	}
			// 	entries.push(Manga {
			// 		key: format!("{i}"),
			// 		title,
			// 		cover: Some(String::from("https://aidoku.app/images/icon.png")),
			// 		authors: Some(vec![String::from("Author")]),
			// 		..Default::default()
			// 	})
			// }


		//TODO: set entries, has_net_page
		// Ok(MangaPageResult{
		// 	entries,
		// 	has_next_page
		// });
		// Err(AidokuError::Unimplemented)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga, 
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_url = format!("{BASE_URL}{}",manga.key);
		let html = Request::get(&manga_url)?.html()?;
		if needs_details{
			//"[q:key=\"kd_0\"]"
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
			let authors:Vec<String> = authors_str
				.split(" ")
				.map(|s| s.to_string())
				.collect();
			manga.authors = Some(authors);
			let manga_tags: Vec<String> = html
				.select("[q:key=\"kd_0\"]")
				.map(|els| {
					els.filter_map(|el| el.text()).collect()
				})
				.unwrap_or_default();

			manga.tags = Some(manga_tags);
			let tags = manga.tags.as_deref().unwrap_or_default();
			manga.content_rating = if tags.as_ref()
				.into_iter()
				.any(|e| matches!(e.as_str(), "Doujinshi" | "Adult" | "Mature" | "Smut"))
			{
				ContentRating::NSFW
			} else if tags.iter().any(|e| e == "Ecchi") {
				ContentRating::Suggestive
			} else {
				ContentRating::Safe
			};
		}

		if needs_chapters{
			manga.chapters = html
				.select("[q:key=\"8t_8\"]")
				.map(|elements| {
					elements
					.filter_map(|element| {
						let links = element
							.select_first("a");
						let url = links
							.as_ref()
							.and_then(|el| el.attr("abs:href"))
							.unwrap_or_default();

						let key = url.strip_prefix(&manga_url).unwrap_or_default().into();
						let title = links
							.as_ref()
							.and_then(|el| el.text());

						let date_uploaded = element
						.select_first("time")
						.and_then(|el| el.attr("data-time"))?
						.parse::<i64>()
						.ok()
						.and_then(|dt| chrono::DateTime::from_timestamp_millis(dt))
						.map(|d| d.timestamp())
						.unwrap_or_default();

						Some(Chapter {
							key,
							title,
							date_uploaded: Some(date_uploaded),
							url: Some(url),
							..Default::default()
						})
					})
				.collect::<Vec<_>>()
			});
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_url = format!("{BASE_URL}{}{}", manga.key, chapter.key);
		let html = Request::get(&chapter_url)?.html()?;
		let mut pages: Vec<Page> = Vec::new();
		let script_str = html
			.select("[type=\"qwik/json\"]")
			.and_then(|el | el.html())
			.unwrap_or_default();
		let chap = script_str
			.find("\"https://s")
			.unwrap_or(0);
		let end = script_str
			.find("whb")
			.unwrap_or(0);
		let mut text_slice = script_str[chap..end].to_string();
		text_slice = text_slice.replace("\"", "");
		text_slice.pop();
		let arr = text_slice
			.split(",")
			.collect::<Vec<_>>();
		for page_url in arr{
			pages.push(Page{
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
			let html = Request::get(format!("{BASE_URL}/latest/{}",page))?.html()?;
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
					.collect::<Vec<_>>()
				})
				.unwrap_or_default();

			Ok(MangaPageResult {
				entries,
				has_next_page: true, 
			})
		} else {
			bail!("Invalid listing");
		}	
	}
	
}

impl Home for MangaPark {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;
		const POPULAR_UPDATES_SELECTOR: & str = "[q:key=\"xL_7\"]"; 
		const MEMBER_UPLOADS_SELECTOR: &str = "[q:key=\"QJ_7\"]";
		const LATEST_RELEASES_SELECTOR: &str = "[q:key=\"Di_7\"]";
		const CHAPTER_SELECTOR: &str = "[q:key=\"R7_8\"]";

		fn parse_manga_with_chapter_with_details(el: &Element)-> Option<MangaWithChapter>{
			let links = el.select_first("a")?;
			let manga_title = el.select("h3 span")?.text()?;
			let cover = el.select_first("img")?.attr("abs:src"); 
			let manga_url = links.attr("abs:href").unwrap_or_default();
			let manga_key:String = manga_url
				.strip_prefix(BASE_URL)?
				.into();
			let ch_el = el.select(CHAPTER_SELECTOR)?;
			let ch_url = ch_el.select_first("a")?.attr("abs:href").unwrap_or_default();
			let ch_title = ch_el.select_first("a > span")?.text().unwrap_or_default();
			let ch_key:String = ch_url
				.strip_prefix(&manga_url)?
				.into();
			let date_uploaded = el
				.select_first("time")
				.and_then(|el| el.attr("data-time"))?
				.parse::<i64>()
				.ok()
				.and_then(|dt| chrono::DateTime::from_timestamp_millis(dt))
				.map(|d| d.timestamp())
				.unwrap_or_default();
			Some(MangaWithChapter 
				{ 
					manga: Manga{ 
						key: manga_key,
						title: manga_title,
						cover,
						url: Some(manga_url),
						// tags: Some(manga_tags), //Change/Edit tags
						..Default::default()
					}, 
					chapter: Chapter{
						key: ch_key,
						title: Some(ch_title),
						date_uploaded: Some(date_uploaded),
						url: Some(ch_url),
						..Default::default()
					}
				})
		}
		
		fn parse_manga(el: &Element) ->  Option<Manga>{
			let manga_url = el.select_first("a")?.attr("abs:href");
			let cover = el.select_first("img")?.attr("abs:src");
			let manga_key:String = manga_url
				.as_ref()?
				.strip_prefix(BASE_URL)?
				.into();
			let title = el.select_first("a.font-bold").unwrap().text().unwrap_or_default();
			Some(Manga{ 
				title,
				key: manga_key,
				cover,
				url: manga_url,
				..Default::default()
			})
		}
		let popular_updates = html
			.select(POPULAR_UPDATES_SELECTOR)
			.map(|els| {
				els.filter_map(|el| parse_manga(&el).map(Into::into)).collect::<Vec<_>>()
			})
			.unwrap_or_default();

		let member_uploads = html
		.select(MEMBER_UPLOADS_SELECTOR)
		.map(|els| {
			els.filter_map(|el| parse_manga_with_chapter_with_details(&el))
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();

		let latest_releases = html
			.select(LATEST_RELEASES_SELECTOR)
			.map(|els| {
				els.filter_map(|el| parse_manga_with_chapter_with_details(&el))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some(("Popular Releases").into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller 
					{ 
						entries: popular_updates,
						listing: None
					},
				},
				HomeComponent {
					title: Some(("Member Uploads").into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList 
					{ 
						page_size: Some(PAGE_SIZE),
						entries: member_uploads,
						listing: None
					},
				},
				HomeComponent {
					title: Some(("Latest Releases").into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList 
					{ 
						page_size: Some(PAGE_SIZE),
						entries: latest_releases, 
						listing: Some(Listing { 
							id: "latest".into(),
							name: "Latest Releases".into(),
							..Default::default()
						}) 
					},
				},
			],
		})
	}
}

impl DeepLinkHandler for MangaPark {
	fn handle_deep_link(&self, _url: String) -> Result<Option<DeepLinkResult>> {
		Err(AidokuError::Unimplemented)
	}
}

register_source!(MangaPark, ListingProvider, Home, DeepLinkHandler);