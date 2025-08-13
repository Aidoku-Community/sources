#![no_std]
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	imports::{html::*, net::Request},
	prelude::*, 
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaWithChapter, Page, Result, Source
};

// mod filter;
/** Build w/ aidoku pkg
 * 
 * 
 */

const BASE_URL: &str = "https://mangapark.com";

const PAGE_SIZE: i32 = 16;

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
		// let manga_url = format!("{BASE_URL}{}", manga.key);
		// println!("{}",manga_url);
		// if needs_details{
		// 	let html = Request::get(&manga_url)?.html()?;
		// }

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

// This is the home page, which can contain various components like scrollers, manga lists, etc.
impl Home for MangaPark {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;
		//parent: er_0
		const MEMBER_UPLOADS_SELECTOR: &str = r#"div[q\:key="er_0"]"#;
		//parent: Xs_4
		const LATEST_UPDATES_SELECTOR: &str = r#"div[q\:key="MY_0"]"#;

		const CHAPTER_SELECTOR: &str = r#"div[q\:key="R7_8"]"#;

		fn parse_manga_with_chapter(element: &Element)-> Option<MangaWithChapter>{
			let links = element.select_first("a")?;
			let manga_title = element.select("h3 span")?.text()?;
			let cover = element.select_first("img")?.attr("abs:src"); 
			let manga_url = links.attr("abs:href");
			println!("{}", links.attr("abs:href").unwrap_or_default());
			let manga_key = links.attr("abs:href")?.strip_prefix("/title")?.strip_suffix("/en")?.into();


			// let chapter_key = 
			Some(MangaWithChapter 
				{ 
					manga: Manga{ 
						key: manga_key,
						title: manga_title,
						cover,
						url: manga_url,
						// content_rating: , // NSFW if Doujinshi... 
						..Default::default()
					}, 
					chapter: Chapter{
						// key: ,
						//title: ,
						//chapter_number: ,
						//date_uploaded: ,

						..Default::default()
					}
				})
		}
//////////////////////////////////////////////////////////////////////////////////////////////////////////////////

		let member_uploads = html
		.select(MEMBER_UPLOADS_SELECTOR)
		.map(|elment_list| {
			elment_list.filter_map(|element| parse_manga_with_chapter(&element))
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();

		// let latest_updates = html
		// 	.select("section:has(h2:contains(Latest Updates)) article")
		// 	.map(|els| {
		// 		els.filter_map(|el| parse_manga_with_chapter(&el))
		// 			.collect::<Vec<_>>()
		// 	})
		// 	.unwrap_or_default();


		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some(("Member Uploads").into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList 
					{ 
						page_size: Some(PAGE_SIZE),
						entries: member_uploads, 
						listing: None
					} 
				},
				HomeComponent {
					title: Some(("Latest Releases").into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList 
					{ 
						page_size: Some(PAGE_SIZE),
						entries: Vec::new(), 
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
