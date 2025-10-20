#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, ImageRequestProvider, Listing, ListingProvider,
	Manga, MangaPageResult, Page, Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use html::{ChapterPage as _, MangaPage as _};
use net::Url;

mod home;
mod html;
mod net;

pub const BASE_URL: &str = "https://www.wnacg01.cc";
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

struct Wnacg;

impl Source for Wnacg {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<aidoku::FilterValue>,
	) -> Result<MangaPageResult> {
		let url = Url::from_query_or_filters(query.as_deref(), page, &filters)?;
		let html = url.request()?.html()?;
		html.manga_page_result()
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let html = Url::manga(manga.key.clone()).request()?.html()?;
			html.manga_details(&mut manga)?;
		}

		if needs_chapters {
			let html = Url::manga(manga.key.clone()).request()?.html()?;
			manga.chapters = Some(html.chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, _chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}", Url::chapter(manga.key));
		let text = Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.string()?;
		let urls = text
			.split("\\\"")
			.filter(|s| s.starts_with("//"))
			.map(|s| format!("https:{}", s))
			.collect::<Vec<String>>();

		let mut pages: Vec<Page> = Vec::new();
		for url in urls.into_iter() {
			pages.push(Page {
				content: aidoku::PageContent::Url(url, None),
				..Default::default()
			});
		}

		Ok(pages)
	}
}

impl ImageRequestProvider for Wnacg {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Wnacg {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let url = url.trim_start_matches(BASE_URL);
		let mut splits = url.split('/').skip(1);
		let deep_link_result = match splits.next() {
			Some("photos-index-aid") => match splits.next() {
				Some(id) => {
					let id = id.trim_end_matches(".html");
					Some(DeepLinkResult::Manga { key: id.into() })
				}
				_ => None,
			},
			_ => None,
		};
		Ok(deep_link_result)
	}
}

impl ListingProvider for Wnacg {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"dayup" => format!(
				"{}/albums-favorite_ranking-page-{}-type-day",
				BASE_URL, page
			),
			"weekup" => format!(
				"{}/albums-favorite_ranking-page-{}-type-week",
				BASE_URL, page
			),
			"monthup" => format!(
				"{}/albums-favorite_ranking-page-{}-type-month",
				BASE_URL, page
			),
			"update" => format!("{}/albums-index-page-{}.html", BASE_URL, page),
			"doujinshi" => format!("{}/albums-index-page-{}-cate-5.html", BASE_URL, page),
			"one-shot" => format!("{}/albums-index-page-{}-cate-6.html", BASE_URL, page),
			"magazine" => format!("{}/albums-index-page-{}-cate-7.html", BASE_URL, page),
			"korean" => format!("{}/albums-index-page-{}-cate-19.html", BASE_URL, page),
			_ => return self.get_search_manga_list(None, page, Vec::new()),
		};

		let html = Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header("Origin", BASE_URL)
			.html()?;

		html.manga_page_result()
	}
}

register_source!(
	Wnacg,
	Home,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
