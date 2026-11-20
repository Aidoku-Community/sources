#![no_std]
mod browse;
mod cipher;
mod filters;
mod helpers;
mod home;
mod models;
mod settings;
mod tags;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, collections::BTreeMap, format, string::ToString, vec},
	imports::{html::Html, std::send_partial_result},
	prelude::*,
};
use browse::BrowseParams;
use helpers::{
	Link, get_html, manga_url, page_data, parse_link, parse_manga, parse_manga_details,
	parse_manga_list, reader, send,
};
use models::{ChapterData, SiteSettings, chapter_list};

const BASE_URL: &str = "https://yaoi.me";

// Reader images are offered at these widths; a wider page is served as the original.
const IMAGE_WIDTHS: [u32; 9] = [90, 140, 200, 260, 360, 500, 720, 1000, 1400];

struct YaoiMe;

impl Source for YaoiMe {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(Link::Series(key)) = query.as_deref().and_then(parse_link)
			&& let Ok(document) = get_html(&manga_url(&key), &SiteSettings::current())
			&& let Some(found) = parse_manga(&document)
		{
			return Ok(MangaPageResult {
				entries: vec![found],
				has_next_page: false,
			});
		}

		let mut search = BrowseParams::new(query);
		for filter in filters {
			search.filter(filter);
		}
		search.page(page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let body = send(&manga_url(&manga.key), &SiteSettings::current())?.get_string()?;
		if needs_details {
			parse_manga_details(&Html::parse_with_url(&body, BASE_URL)?, &mut manga);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let data: ChapterData = page_data(&body, "{\"groups\":[")?;
			let clean = settings::clean_chapters();
			// Only the reader names the sources, and a clean list shows none; the names are
			// optional, so a chapter the reader refuses doesn't hide the list.
			let names = match data.first_key() {
				Some(key) if !clean => reader(&key)
					.map(|chapter| chapter.sources)
					.unwrap_or_default(),
				_ => BTreeMap::new(),
			};
			manga.chapters = Some(chapter_list(data, &settings::languages(), &names, clean));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let mut chapter = reader(&chapter.key)?;
		let hashes = match chapter.sealed.take() {
			Some(sealed) => cipher::unseal(&sealed)?,
			None => chapter
				.pages
				.iter_mut()
				.map(|page| core::mem::take(&mut page.hash))
				.collect(),
		};
		if chapter.pages.is_empty()
			|| hashes.len() != chapter.pages.len()
			|| hashes.iter().any(|hash| hash.is_empty())
		{
			bail!("This chapter has missing pages");
		}
		Ok(chapter
			.pages
			.iter()
			.zip(hashes)
			.map(|(page, hash)| {
				let width = page
					.w
					.and_then(|w| IMAGE_WIDTHS.iter().find(|width| **width >= w));
				let url = match width {
					// A page hash is plain hex, so this url has no query before ours.
					Some(width) => format!("{BASE_URL}/i/p/{hash}?w={width}"),
					None => format!("{BASE_URL}/i/p/{hash}"),
				};
				Page {
					content: PageContent::url(url),
					..Default::default()
				}
			})
			.collect())
	}
}

impl ListingProvider for YaoiMe {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if listing.id == "latest" {
			return BrowseParams::new(None).page(page);
		}
		if !home::SECTIONS.iter().any(|(id, ..)| *id == listing.id) {
			bail!("Unknown listing");
		}
		let settings = SiteSettings::current();
		if settings.shows_nothing() {
			return Ok(MangaPageResult::default());
		}
		// Discover pages list their whole shelf at once.
		let document = get_html(&format!("{BASE_URL}/discover/{}", listing.id), &settings)?;
		Ok(MangaPageResult {
			entries: document
				.select_first("main")
				.map(|main| parse_manga_list(&main))
				.unwrap_or_default(),
			has_next_page: false,
		})
	}
}

impl DeepLinkHandler for YaoiMe {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(match parse_link(&url) {
			// A series opens under the id of any of its sources; the page names the title's own.
			Some(Link::Series(key)) => {
				let document = get_html(&manga_url(&key), &SiteSettings::current())?;
				parse_manga(&document).map(|manga| DeepLinkResult::Manga { key: manga.key })
			}
			Some(Link::Chapter(key)) => {
				reader(&key)?.title_id.map(|title| DeepLinkResult::Chapter {
					manga_key: title.to_string(),
					key,
				})
			}
			None => None,
		})
	}
}

register_source!(
	YaoiMe,
	Home,
	ListingProvider,
	DynamicFilters,
	DynamicSettings,
	DeepLinkHandler
);
