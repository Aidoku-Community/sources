#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, Link, Listing, ListingProvider, Manga, MangaPageResult, MangaWithChapter, Page,
	PageContent, Result, Source,
	alloc::{String, Vec, vec},
	imports::{
		net::{TimeUnit, set_rate_limit},
		std::send_partial_result,
	},
	prelude::*,
};

mod helpers;
mod parser;
use helpers::{
	BASE_URL, LibraryParams, chapter_key, chapter_url, fetch, fetch_html, latest_listing,
	library_url, manga_key, manga_url, popular_listing,
};
use parser::{
	parse_chapters, parse_hero, parse_home_section, parse_library, parse_manga_cards,
	parse_manga_details, parse_page_urls, parse_ranking, parse_reader_manga_key,
	parse_recent_chapters,
};

/// Rows per page of the home page lists, small enough for small screens.
const HOME_PAGE_SIZE: i32 = 3;

struct Rncalation;

impl Source for Rncalation {
	fn new() -> Self {
		set_rate_limit(2, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut params = LibraryParams::sorted("latest");
		params.query = query.as_deref().map(str::trim).filter(|q| !q.is_empty());
		for filter in &filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"sort" => params.sort = value,
					"type" => params.kind = value,
					"status" => params.status = value,
					"genre" => params.genre = value,
					_ => {}
				}
			}
		}
		Ok(parse_library(&fetch_html(&library_url(&params, page))?))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			parse_manga_details(&fetch_html(&manga_url(&manga.key))?, &mut manga);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			// The chapter list comes in pages of 30, with the page count in a header.
			let mut chapters = Vec::new();
			let mut page = 1;
			loop {
				let url = format!("{BASE_URL}/comics/{}/chapters?page={page}", manga.key);
				let (response, html) = fetch(&url)?;
				chapters.extend(parse_chapters(&html));
				let pages = response
					.get_header("x-pages")
					.and_then(|pages| pages.parse::<i32>().ok())
					.unwrap_or(1);
				if page >= pages {
					break;
				}
				page += 1;
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let urls = parse_page_urls(&fetch_html(&chapter_url(&chapter.key))?);
		if urls.is_empty() {
			bail!("No se encontraron las páginas del capítulo");
		}
		Ok(urls
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for Rncalation {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"Popular" => "views",
			"Latest" => "updated",
			_ => bail!("Listado no válido: {}", listing.id),
		};
		let url = library_url(&LibraryParams::sorted(sort), page);
		Ok(parse_library(&fetch_html(&url)?))
	}
}

fn links(entries: Vec<Manga>) -> Vec<Link> {
	entries.into_iter().map(Link::from).collect()
}

impl Home for Rncalation {
	fn get_home(&self) -> Result<HomeLayout> {
		// Every section comes from the home page, in the same order.
		let html = fetch_html(BASE_URL)?;

		let components = vec![
			HomeComponent {
				title: None,
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: parse_hero(&html),
					auto_scroll_interval: Some(8.0),
				},
			},
			HomeComponent {
				title: Some("Más populares".into()),
				subtitle: Some("Hoy".into()),
				value: HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(HOME_PAGE_SIZE),
					entries: links(parse_ranking(&html)),
					listing: Some(popular_listing()),
				},
			},
			HomeComponent {
				title: Some("Tendencias".into()),
				subtitle: Some("Hoy".into()),
				value: HomeComponentValue::Scroller {
					entries: links(parse_manga_cards(
						&html,
						".trending-panel[data-panel=today] a.comic-card",
					)),
					listing: Some(popular_listing()),
				},
			},
			HomeComponent {
				title: Some("Recién actualizados".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: links(parse_home_section(&html, "Recién actualizados")),
					listing: Some(latest_listing()),
				},
			},
			HomeComponent {
				title: Some("Nuevos cómics".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: links(parse_home_section(&html, "Nuevos cómics")),
					listing: None,
				},
			},
			HomeComponent {
				title: Some("Actualizados recientemente".into()),
				subtitle: None,
				value: HomeComponentValue::MangaChapterList {
					page_size: Some(HOME_PAGE_SIZE),
					entries: parse_recent_chapters(&html)
						.into_iter()
						.map(|(manga, chapter)| MangaWithChapter { manga, chapter })
						.collect(),
					listing: Some(latest_listing()),
				},
			},
		];
		Ok(HomeLayout { components })
	}
}

impl DeepLinkHandler for Rncalation {
	/// Handles:
	///   /comics/{slug}  -> the series
	///   /leer/{code}    -> the chapter
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(key) = manga_key(&url) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}

		let Some(key) = chapter_key(&url) else {
			return Ok(None);
		};
		// The reader url doesn't include the series, so read it from the page.
		let manga_key = parse_reader_manga_key(&fetch_html(&chapter_url(&key))?)
			.ok_or_else(|| error!("Serie no encontrada"))?;
		Ok(Some(DeepLinkResult::Chapter { manga_key, key }))
	}
}

register_source!(Rncalation, ListingProvider, Home, DeepLinkHandler);
