#![no_std]
use aidoku::{
	alloc::{string::ToString, vec, String, Vec},
	imports::{html::*, net::Request},
	prelude::*,
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, 
	Home, HomeComponent, HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus,
	MangaWithChapter, Page, PageContent, Result, Source
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
		let html = Request::get(BASE_URL)?.html()?;

		const POPULAR_UPDATES_SELECT: & str = "[q:key=\"FN_0\"]"; //todo
		//parent: er_0, alternatively each element = QJ_7
		//if I wanted to use parent I would need to select on (MEMBER_UPLOADS_SELECTOR div) which basically selects child div elements.
		const MEMBER_UPLOADS_SELECTOR: &str = "[q:key=\"QJ_7\"]";

		const MANGA_TAGS_SELECTOR: &str = "[q:key=\"kd_0\"]";
		//parent: Xs_4
		//
		const LATEST_RELEASES_SELECTOR: &str = "[q:key=\"Di_7\"]";

		const CHAPTER_SELECTOR: &str = "[q:key=\"R7_8\"]";

		//Example URL: https://mangapark.com/title/425080-en-the-fake-master-who-accidentally-became-the-strongest/9807869-chapter-9
		const KEY_INDEX: usize = 0; 
		const MANGA_PATH_SEGMENT_INDEX: usize = 4;
		const CHAPTER_PATH_SEGMENT_INDEX: usize = 5;

		fn parse_manga_with_chapter(element: &Element)-> Option<MangaWithChapter>{

			//Example URL: https://mangapark.com/title/425080-en-the-fake-master-who-accidentally-became-the-strongest/9807869-chapter-9
			println!("Parse Manga with Chapter");
			let links = element.select_first("a")?;
			let manga_title = element.select("h3 span")?.text()?;
			let cover = element.select_first("img")?.attr("abs:src"); 
			let manga_url = links.attr("abs:href").unwrap_or_default();
			
			let parts:Vec<&str> = manga_url.split("/").collect();
			let manga_key_and_title = parts[MANGA_PATH_SEGMENT_INDEX].to_string(); 
			let split_key_and_title:Vec<&str> = manga_key_and_title.split("-").collect();
			let manga_key = split_key_and_title[KEY_INDEX].to_string();

			let manga_tags_str = element.select(MANGA_TAGS_SELECTOR)?.text()?;
			let tags:Vec<&str> = manga_tags_str.split(" ").collect();
			let manga_tags:Vec<String> = tags.
											iter()
											.map(|s| s.to_string())
											.collect();

			//Chapter Tags PARENT SELECTOR: R7_8
			let chapter_element = element.select(CHAPTER_SELECTOR)?;
			let chapter_url = chapter_element.select_first("a")?.attr("abs:href").unwrap_or_default();

			let parts:Vec<&str> = chapter_url.split("/").collect();
			let chapter_key_and_title = parts[CHAPTER_PATH_SEGMENT_INDEX].to_string(); 
			let split_key_and_title:Vec<&str> = chapter_key_and_title.split("-").collect();
			//
			let chapter_key = split_key_and_title[KEY_INDEX].to_string();
			let chapter_number = split_key_and_title[2].to_string().parse::<f32>().unwrap_or_default();
			
			let date_uploaded = element
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
						tags: Some(manga_tags),
						// content_rating: , // NSFW if Doujinshi... 
						..Default::default()
					}, 
					chapter: Chapter{
						key: chapter_key,
						chapter_number: Some(chapter_number),
						date_uploaded: Some(date_uploaded),
						url: Some(chapter_url),
						..Default::default()
					}
				})
		}
		
		// let popular_updates = html
		// 	.select(CHAPTER_SELECTOR)
		// 	.map(|els| {
		// 		els.filter_map(|el| parse_manga_with_chapter(&el))
		// 			.collect::<Vec<_>>()
		// 	})
		// 	.unwrap_or_default();

		let member_uploads = html
		.select(MEMBER_UPLOADS_SELECTOR)
		.map(|element_list| {
			element_list.filter_map(|element| parse_manga_with_chapter(&element))
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();

		let latest_releases = html
			.select(LATEST_RELEASES_SELECTOR)
			.map(|els| {
				els.filter_map(|el| parse_manga_with_chapter(&el))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

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
					},
				},
				HomeComponent {
					title: Some(("Latest Releases").into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::MangaChapterList 
					{ 
						page_size: Some(PAGE_SIZE),
						entries: latest_releases, 
						listing: None,
						// listing: Some(Listing { 
						// 	id: "latest".into(),
						// 	name: "Latest Releases".into(),
						// 	..Default::default()
						// }) 
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