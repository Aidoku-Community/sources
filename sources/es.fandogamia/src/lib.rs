#![no_std]
mod helper;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, Link, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent, Result,
	Source,
	alloc::{String, Vec, vec},
	imports::net::Request,
	prelude::*,
};
use helper::{
	BASE_URL, fetch_series_chapters, fetch_series_list, parse_page_image_url, slug_from_url,
};

struct Fandogamia;

impl Source for Fandogamia {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut entries = fetch_series_list()?;
		if let Some(query) = query {
			let query = query.to_ascii_lowercase();
			entries.retain(|manga| manga.title.to_ascii_lowercase().contains(&query));
		}

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details
			&& let Some(details) = fetch_series_list()?
				.into_iter()
				.find(|series| series.key == manga.key)
		{
			manga.copy_from(details);
		}
		if needs_chapters {
			manga.chapters = Some(fetch_series_chapters(&manga.key)?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = match chapter.url {
			Some(url) => url,
			// The host doesn't always carry `chapter.url` across calls (e.g. when a
			// page fetch is triggered later from just the manga/chapter keys), so
			// fall back to re-fetching the series' chapter list and matching by key.
			None => fetch_series_chapters(&manga.key)?
				.into_iter()
				.find(|c| c.key == chapter.key)
				.and_then(|c| c.url)
				.ok_or_else(|| error!("Strip URL not found"))?,
		};
		let html = Request::get(url)?.html()?;
		let image_url =
			parse_page_image_url(&html).ok_or_else(|| error!("Strip URL not found in page"))?;

		Ok(vec![Page {
			content: PageContent::url(image_url),
			..Default::default()
		}])
	}
}

impl ListingProvider for Fandogamia {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.get_search_manga_list(None, page, Vec::new())
	}
}

impl Home for Fandogamia {
	fn get_home(&self) -> Result<HomeLayout> {
		let entries = fetch_series_list()?.into_iter().map(Link::from).collect();

		Ok(HomeLayout {
			components: vec![HomeComponent {
				title: Some(String::from("Series")),
				subtitle: None,
				value: HomeComponentValue::MangaList {
					ranking: false,
					page_size: None,
					entries,
					listing: None,
				},
			}],
		})
	}
}

impl DeepLinkHandler for Fandogamia {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}

		let path = url.trim_start_matches(BASE_URL);
		let segments = path
			.split('/')
			.filter(|part| !part.is_empty())
			.collect::<Vec<_>>();

		match segments.as_slice() {
			["archive", slug] => Ok(Some(DeepLinkResult::Manga {
				key: String::from(*slug),
			})),
			// "/comic/<slug>/<id>/<title>": a chapter within a specific series
			["comic", slug, id, ..]
				if !slug.chars().all(|c| c.is_ascii_digit())
					&& id.chars().all(|c| c.is_ascii_digit()) =>
			{
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: String::from(*slug),
					key: slug_from_url(&url),
				}))
			}
			// "/comic/<slug>": a series' home page
			["comic", slug] if !slug.chars().all(|c| c.is_ascii_digit()) => {
				Ok(Some(DeepLinkResult::Manga {
					key: String::from(*slug),
				}))
			}
			// "/comic/<id>/<title>" and "/archive/" don't carry enough information
			// to resolve which series they belong to
			_ => Ok(None),
		}
	}
}

register_source!(Fandogamia, ListingProvider, Home, DeepLinkHandler);
