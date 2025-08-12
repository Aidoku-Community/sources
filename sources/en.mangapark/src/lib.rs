#![no_std]
use aidoku::{
	alloc::{vec, String, Vec},
	imports::{defaults::defaults_get, net::Request},
	prelude::*, 
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaWithChapter, Page, Result, Source
};

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
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// URL with search/filter options to get list of manga... https://mangapark.com/search?
		let url = format!("{}/search", BASE_URL);

		// List of Manga in Selector: q:key="jp_1"
		// div[q\:key="jp_1"]

		const LIST_SELECTOR: &str = r#"div[q\:key="jp_1"]"#;
		const MANGA_SELECTOR: &str = r#"div[q\:key="q4_9"]"#;

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
		mut manga: Manga, //From weebcentral
		// _manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		//Ex: https://mangapark.com/title/346393-en-an-introvert-s-hookup-hiccups-this-gyaru-is-head-over-heels-for-me
		// what is the page component suppose to be?
		// let url = format!("{}/search/{}-", BASE_URL,page,);
// 
		let manga_url = format!("{BASE_URL}{}", manga.key);
		println!("{}",manga_url);
		if needs_details{
			let html = Request::get(&manga_url)?.html()?;
		}

		if needs_chapters{

		}
		Err(AidokuError::Unimplemented)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Err(AidokuError::Unimplemented)
	}
}

// Listing for /latest
impl ListingProvider for MangaPark {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		Err(AidokuError::Unimplemented)
	}
}

impl Home for MangaPark {
	fn get_home(&self) -> Result<HomeLayout> {
		let entries = self.get_search_manga_list(None, 1, Vec::new())?.entries;
		let chapter = Chapter {
			key: String::from("1"),
			chapter_number: Some(1.0),
			title: Some(String::from("Chapter")),
			date_uploaded: Some(1692318525),
			..Default::default()
		};
		let manga_chapters = entries
			.iter()
			.map(|manga| MangaWithChapter {
				manga: manga.clone(),
				chapter: chapter.clone(),
			})
			.take(3)
			.collect::<Vec<_>>();
		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some(String::from("Big Scroller")),
					subtitle: None,
					value: aidoku::HomeComponentValue::BigScroller {
						entries: entries.clone(),
						auto_scroll_interval: Some(10.0),
					},
				},
				HomeComponent {
					title: Some(String::from("Manga Chapter List")),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList {
						page_size: None,
						entries: manga_chapters,
						listing: None,
					},
				},
				HomeComponent {
					title: Some(String::from("Manga List")),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaList {
						ranking: false,
						page_size: None,
						entries: entries.iter().take(2).cloned().map(|m| m.into()).collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some(String::from("Manga List (Paged, Ranking)")),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaList {
						ranking: true,
						page_size: Some(3),
						entries: entries.iter().take(8).cloned().map(|m| m.into()).collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some(String::from("Scroller")),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: entries.clone().into_iter().map(|m| m.into()).collect(),
						listing: None,
					},
				},
				HomeComponent {
					title: Some("Filters".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Filters(vec![
						aidoku::FilterItem::from(String::from("Action")),
						"Adventure".into(),
						"Fantasy".into(),
						"Horror".into(),
						"Slice of Life".into(),
						"Magic".into(),
						"Adaptation".into(),
					]),
				},
				HomeComponent {
					title: Some(String::from("Links")),
					subtitle: None,
					value: aidoku::HomeComponentValue::Links(vec![
						aidoku::Link {
							title: String::from("Website Link"),
							value: Some(aidoku::LinkValue::Url(String::from("https://aidoku.app"))),
							..Default::default()
						},
						aidoku::Link {
							title: String::from("Manga Link"),
							value: Some(aidoku::LinkValue::Manga(entries.first().unwrap().clone())),
							..Default::default()
						},
						aidoku::Link {
							title: String::from("Listing Link"),
							value: Some(aidoku::LinkValue::Listing(Listing {
								id: String::from("listing"),
								name: String::from("Listing"),
								kind: aidoku::ListingKind::List,
							})),
							..Default::default()
						},
					]),
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
