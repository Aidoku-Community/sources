#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, vec},
	imports::{
		net::Request,
		std::{current_date, send_partial_result},
	},
	prelude::*,
};

mod helpers;
mod home;
mod models;
mod reader;
mod rsc;
mod settings;

use helpers::{browse, chapter_url, get_html, link_filters, manga_url, parse_key, parse_response};
use models::ReaderProps;
use rsc::{chapter_list, extract};
use settings::{Catalogue, Groups};

const BASE_URL: &str = "https://webnex.cc";

struct Webnex;

impl Source for Webnex {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// A link that fails to resolve is searched as text.
		if let Some(key) = query
			.as_deref()
			.and_then(|url| parse_key(url.strip_prefix(BASE_URL)?, "/manga/"))
			&& let Ok(Manga {
				key,
				title,
				cover,
				content_rating,
				status,
				..
			}) = get_html(&manga_url(key), None).and_then(|doc| models::details(&doc, key))
		{
			return Ok(MangaPageResult {
				entries: vec![Manga {
					key,
					title,
					cover,
					content_rating,
					status,
					..Default::default()
				}],
				has_next_page: false,
			});
		}
		browse(query.as_deref(), page, filters, Catalogue::load())
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = manga_url(&manga.key);
		let doc = get_html(&url, None)?;
		if doc.select_first("h1.series-title").is_none() {
			bail!("Webnex could not load this series");
		}
		if needs_details {
			manga.copy_from(models::details(&doc, &manga.key)?);
		}
		if needs_chapters {
			let doc = chapter_list(&doc)?;
			let last = doc
				.select("a[href*='page='][href$='#chapters']")
				.into_iter()
				.flatten()
				.filter_map(|el| {
					el.attr("href")?
						.split_once("page=")?
						.1
						.split(['&', '#'])
						.next()?
						.parse::<usize>()
						.ok()
				})
				.max()
				.unwrap_or(1);
			if needs_details && last > 1 {
				send_partial_result(&manga);
			}
			let groups = Groups::load();
			let now = current_date();
			let mut uploads = models::uploads(&doc, &groups, now)?;
			let pages: Vec<_> = (2..=last).collect();
			for batch in pages.chunks(4) {
				let reqs = batch
					.iter()
					// The series url has no query of its own.
					.map(|page| Request::get(format!("{url}?page={page}")))
					.collect::<core::result::Result<Vec<_>, _>>()?;
				for response in Request::send_all(reqs) {
					let page = chapter_list(&parse_response(&response?)?)?;
					if page.select_first(".ch-row").is_none() {
						bail!("Webnex did not return the requested chapter page");
					}
					uploads.extend(models::uploads(&page, &groups, now)?);
				}
			}
			manga.chapters = Some(models::select_chapters(uploads, &groups));
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		reader::pages(&chapter.key)
	}
}

impl DeepLinkHandler for Webnex {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		if let Some(key) = parse_key(path, "/manga/") {
			return Ok(Some(DeepLinkResult::Manga { key: key.into() }));
		}
		let Some(key) = parse_key(path, "/read/") else {
			return Ok(None);
		};
		let reader = extract::<ReaderProps>(
			&get_html(&chapter_url(key), None)?,
			&["chapter", "series", "pagesUrl"],
		)
		.ok_or_else(|| error!("Webnex did not identify the chapter's series"))?;
		Ok(
			parse_key(&reader.series.href, "/manga/").map(|s| DeepLinkResult::Chapter {
				manga_key: s.into(),
				key: key.into(),
			}),
		)
	}
}

impl ListingProvider for Webnex {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.get_search_manga_list(None, page, link_filters(&listing.id)?)
	}
}

impl ImageRequestProvider for Webnex {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

register_source!(
	Webnex,
	Home,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
