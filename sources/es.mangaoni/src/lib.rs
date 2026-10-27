#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, HomePartialResult, Link, Listing, ListingProvider, Manga, MangaPageResult,
	MangaWithChapter, Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
	helpers::uri::encode_uri_component,
	imports::{defaults::defaults_get, net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;
mod parser;
use helpers::{
	BASE_URL, DirectoryParams, chapter_key, chapter_url, directory_url, manga_key,
	manga_key_from_reader, path_segments,
};
use parser::{
	has_next_page, image_url, parse_chapters, parse_latest_updates, parse_manga_card,
	parse_manga_cards, parse_manga_details, parse_page_urls,
};

const HIDE_NSFW_SETTING: &str = "hideNsfw";

fn hide_nsfw() -> bool {
	defaults_get::<bool>(HIDE_NSFW_SETTING).unwrap_or(false)
}

struct MangaOni;

impl Source for MangaOni {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// The site's title search doesn't take filters, so they're hidden while
		// searching (`hidesFiltersWhileSearching`).
		if let Some(query) = query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
			let url = format!(
				"{BASE_URL}/buscar?q={}&p={page}",
				encode_uri_component(query)
			);
			return Ok(parse_manga_cards(&Request::get(url)?.html()?));
		}

		let mut params = DirectoryParams::popular(false);
		for filter in &filters {
			match filter {
				FilterValue::Select { id, value } => match id.as_str() {
					"genero" => params.genre = value,
					"estado" => params.status = value,
					"tipo" => params.kind = value,
					"adulto" => params.adult = value,
					_ => {}
				},
				FilterValue::Sort {
					index, ascending, ..
				} => {
					params.sort = match index {
						1 => "id",
						2 => "nombre",
						_ => "visitas",
					};
					params.order = if *ascending { "asc" } else { "desc" };
				}
				_ => {}
			}
		}
		if hide_nsfw() {
			params.adult = "0";
		}

		let url = directory_url(&params, page);
		Ok(parse_manga_cards(&Request::get(url)?.html()?))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// Details and chapters are on the same page.
		let html = Request::get(format!("{BASE_URL}{}", manga.key))?.html()?;

		if needs_details {
			parse_manga_details(&html, &mut manga);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(parse_chapters(&html));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = chapter_url(&manga.key, &chapter.key);
		Ok(parse_page_urls(&Request::get(url)?.html()?)?
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for MangaOni {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"Popular" => {
				let url = directory_url(&DirectoryParams::popular(hide_nsfw()), page);
				Ok(parse_manga_cards(&Request::get(url)?.html()?))
			}
			"Latest" => {
				let html = Request::get(format!("{BASE_URL}/recientes?p={page}"))?.html()?;
				Ok(MangaPageResult {
					entries: parse_latest_updates(&html)
						.into_iter()
						.map(|(manga, _)| manga)
						.collect(),
					has_next_page: has_next_page(&html),
				})
			}
			_ => bail!("Listado no válido: {}", listing.id),
		}
	}
}

fn popular_listing() -> Listing {
	Listing {
		id: "Popular".into(),
		name: "Populares".into(),
		..Default::default()
	}
}

fn latest_listing() -> Listing {
	Listing {
		id: "Latest".into(),
		name: "Últimas actualizaciones".into(),
		..Default::default()
	}
}

impl Home for MangaOni {
	fn get_home(&self) -> Result<HomeLayout> {
		// Send the empty sections first so the app can show them right away.
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Top manga".into()),
					subtitle: None,
					value: HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("Últimas actualizaciones".into()),
					subtitle: None,
					value: HomeComponentValue::empty_manga_chapter_list(),
				},
				HomeComponent {
					title: Some("Nuevos".into()),
					subtitle: None,
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Populares".into()),
					subtitle: None,
					value: HomeComponentValue::empty_scroller(),
				},
			],
		}));

		// The first three sections come from the site's home page.
		let html = Request::get(BASE_URL)?.html()?;

		let top: Vec<Link> = html
			.select("#widget.top li")
			.map(|items| {
				items
					.filter_map(|item| {
						let link = item.select_first("a.chart-title[href]")?;
						let key = manga_key(&link.attr("abs:href")?)?;
						Some(
							Manga {
								url: Some(format!("{BASE_URL}{key}")),
								key,
								title: link.text()?,
								// The widget uses a small thumbnail; the cover is next to it.
								cover: item
									.select_first("img")
									.and_then(|img| image_url(&img))
									.map(|thumb| thumb.replacen("/thumb.", "/cover.", 1)),
								..Default::default()
							}
							.into(),
						)
					})
					.collect()
			})
			.unwrap_or_default();
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Top manga".into()),
			subtitle: None,
			value: HomeComponentValue::MangaList {
				ranking: true,
				page_size: Some(5),
				entries: top,
				listing: None,
			},
		}));

		let latest = parse_latest_updates(&html)
			.into_iter()
			.filter_map(|(manga, chapter)| {
				Some(MangaWithChapter {
					manga,
					chapter: chapter?,
				})
			})
			.collect();
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Últimas actualizaciones".into()),
			subtitle: None,
			value: HomeComponentValue::MangaChapterList {
				page_size: Some(4),
				entries: latest,
				listing: Some(latest_listing()),
			},
		}));

		// "Nuevos" is the `#article-div` section whose heading says so.
		let new: Vec<Link> = html
			.select("#article-div")
			.and_then(|mut sections| {
				sections.find(|section| {
					section
						.select_first("h3")
						.and_then(|h3| h3.text())
						.is_some_and(|title| title.contains("Nuevos"))
				})
			})
			.and_then(|section| section.select("div._135yj"))
			.map(|cards| {
				cards
					.filter_map(|card| parse_manga_card(&card))
					.map(Link::from)
					.collect()
			})
			.unwrap_or_default();
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Nuevos".into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: new,
				listing: None,
			},
		}));

		let url = directory_url(&DirectoryParams::popular(hide_nsfw()), 1);
		let popular = parse_manga_cards(&Request::get(url)?.html()?)
			.entries
			.into_iter()
			.map(Link::from)
			.collect();
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Populares".into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: popular,
				listing: Some(popular_listing()),
			},
		}));

		// Everything was already sent as partial results.
		Ok(HomeLayout::default())
	}
}

impl DeepLinkHandler for MangaOni {
	/// Handles:
	///   /{manga|manhwa|manhua|oneshot|novela}/{slug}/  -> the series
	///   /lector/{slug}/{chapter id}/[cascada/]          -> the chapter
	///   /leer/{slug}/{chapter id}/                      -> the chapter (novels)
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(key) = manga_key(&url) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}

		let Some(key) = chapter_key(&url) else {
			return Ok(None);
		};
		// Reader urls don't include the series type, so read it from the page.
		let segments = path_segments(&url);
		let reader_url = format!("{BASE_URL}/{}/{}/{key}/", segments[0], segments[1]);
		Ok(Some(DeepLinkResult::Chapter {
			manga_key: manga_key_from_reader(&reader_url)?,
			key,
		}))
	}
}

register_source!(MangaOni, ListingProvider, Home, DeepLinkHandler);
