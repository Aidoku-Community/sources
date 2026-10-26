#![no_std]
use aidoku::{
	BaseUrlProvider, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, Home,
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Listing, ListingProvider,
	Manga, MangaPageResult, MangaWithChapter, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::{defaults::defaults_get, net::Request, std::send_partial_result},
	prelude::*,
};
use core::cell::RefCell;
use serde::de::DeserializeOwned;

mod helpers;
mod models;
use helpers::{detect_base_url, get_json, latest_url, manga_links, parse_manga_id, rankings_url};
use models::{
	ChaptersResponse, DetailsResponse, LatestItemDto, MangaListItemDto, Paginated,
	SeriesListResponse, SeriesResponse,
};

/// Fallback when the domain can't be detected and none is set in settings.
const DEFAULT_BASE_URL: &str = "https://olympusxyz.com";
const BASE_URL_SETTING: &str = "baseUrl";

const SEARCH_PAGE_SIZE: usize = 20;

struct OlympusScanlation {
	/// Every comic on the site, from `/api/series/list`. Used for search, since the
	/// API can't search by title. Loaded on first use.
	series: RefCell<Option<Vec<MangaListItemDto>>>,
	/// Series id -> slug. Keys are ids, but the API only accepts slugs, which change
	/// over time. Updated from every response that includes slugs.
	slugs: RefCell<HashMap<i32, String>>,
	/// Domain found by `detect_base_url`, kept for the rest of the session.
	detected_base_url: RefCell<Option<String>>,
}

impl Source for OlympusScanlation {
	fn new() -> Self {
		Self {
			series: RefCell::new(None),
			slugs: RefCell::new(HashMap::new()),
			detected_base_url: RefCell::new(None),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// Filters are hidden while searching (`hidesFiltersWhileSearching`), since
		// title search runs on the cached series list.
		if let Some(query) = query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
			return self.search_by_title(query, page);
		}

		let mut url = format!("{}/api/series?type=comic&page={page}", self.base_url());
		for filter in filters {
			match filter {
				// Estado and Género; their ids in filters.json match the query parameter
				// names. An empty value means "Todos".
				FilterValue::Select { id, value } if !value.is_empty() => {
					url.push_str(&format!("&{id}={value}"));
				}
				FilterValue::Sort { ascending, .. } => {
					let direction = if ascending { "asc" } else { "desc" };
					url.push_str(&format!("&direction={direction}"));
				}
				_ => {}
			}
		}

		let series = get_json::<SeriesResponse>(&url)?.data.series;
		self.remember_slugs(&series.data);
		Ok(series.into())
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let id = parse_manga_id(&manga.key)?;
		let mut slug = self.slug_for(id)?;
		let base_url = self.base_url();

		if needs_details {
			let details: DetailsResponse =
				self.series_api(&base_url, id, &mut slug, "?type=comic")?;
			manga = details
				.data
				.into_manga(manga, format!("{base_url}/series/comic-{slug}"));

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let mut chapters = Vec::new();
			let mut page = 1;
			loop {
				let response: ChaptersResponse = self.series_api(
					&base_url,
					id,
					&mut slug,
					&format!("/chapters?page={page}&direction=desc&type=comic"),
				)?;
				// Also stop on an empty page, in case `last_page` is ever wrong.
				let done = response.data.is_empty() || page >= response.meta.last_page;
				chapters.extend(
					response
						.data
						.into_iter()
						.map(|chapter| chapter.into_chapter(&base_url, &slug)),
				);
				if done {
					break;
				}
				page += 1;
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let slug = self.slug_for(parse_manga_id(&manga.key)?)?;
		// The site redirects this to the chapter's reader page. Only the chapter id
		// matters, so a slug that changed in the meantime still works.
		let url = format!("{}/capitulo/{}/comic-{slug}", self.base_url(), chapter.key);

		Request::get(&url)?
			.html()?
			.select("main.strip div.panel > img")
			.map(|images| {
				images
					.filter_map(|img| {
						Some(Page {
							content: PageContent::url(img.attr("abs:src")?),
							..Default::default()
						})
					})
					.collect()
			})
			.ok_or_else(|| error!("No se encontraron las páginas del capítulo"))
	}
}

impl OlympusScanlation {
	/// The URL from settings if there is one, otherwise the detected domain.
	fn base_url(&self) -> String {
		if let Some(url) = defaults_get::<String>(BASE_URL_SETTING) {
			let url = url.trim().trim_end_matches('/');
			if !url.is_empty() {
				return url.into();
			}
		}

		if let Some(url) = self.detected_base_url.borrow().as_ref() {
			return url.clone();
		}

		// Cache the fallback too, so a failed detection isn't retried on every request.
		let url = detect_base_url().unwrap_or_else(|| DEFAULT_BASE_URL.into());
		*self.detected_base_url.borrow_mut() = Some(url.clone());
		url
	}

	fn remember_slug(&self, id: i32, slug: &str) {
		let mut slugs = self.slugs.borrow_mut();
		if slugs.get(&id).map(String::as_str) != Some(slug) {
			slugs.insert(id, slug.into());
		}
	}

	fn remember_slugs(&self, items: &[MangaListItemDto]) {
		for item in items {
			self.remember_slug(item.id, &item.slug);
		}
	}

	fn refresh_series(&self) -> Result<()> {
		let url = format!("{}/api/series/list", self.base_url());
		let comics: Vec<MangaListItemDto> = get_json::<SeriesListResponse>(&url)?
			.data
			.into_iter()
			.filter(MangaListItemDto::is_comic)
			.collect();
		self.remember_slugs(&comics);
		*self.series.borrow_mut() = Some(comics);
		Ok(())
	}

	fn slug_for(&self, id: i32) -> Result<String> {
		if let Some(slug) = self.slugs.borrow().get(&id) {
			return Ok(slug.clone());
		}
		// Not seen in this session yet (e.g. a series opened from the library), so
		// load the full list once.
		self.refresh_series()?;
		self.slugs
			.borrow()
			.get(&id)
			.cloned()
			.ok_or_else(|| error!("Serie no encontrada: {id}"))
	}

	/// Requests `/api/series/{slug}{rest}`. A 404 means the slug changed since it was
	/// cached, so the series list is reloaded and the request retried once with the
	/// new slug. `slug` is updated in that case.
	fn series_api<T: DeserializeOwned>(
		&self,
		base_url: &str,
		id: i32,
		slug: &mut String,
		rest: &str,
	) -> Result<T> {
		let response = Request::get(format!("{base_url}/api/series/{slug}{rest}"))?.send()?;
		if response.status_code() != 404 {
			return response.get_json_owned();
		}

		self.refresh_series()?;
		let current = self
			.slugs
			.borrow()
			.get(&id)
			.filter(|current| *current != slug)
			.cloned()
			.ok_or_else(|| error!("Serie no encontrada: {id}"))?;
		*slug = current;
		get_json(&format!("{base_url}/api/series/{slug}{rest}"))
	}

	/// Turns a slug from a URL into a manga key (the series id).
	fn key_for_slug(&self, slug: &str) -> Result<String> {
		let url = format!("{}/api/series/{slug}?type=comic", self.base_url());
		let response = Request::get(url)?.send()?;
		if response.status_code() == 404 {
			// Old links can point to a slug that no longer exists.
			bail!("Serie no encontrada: {slug}");
		}
		let series = response.get_json_owned::<DetailsResponse>()?.data;
		self.remember_slug(series.id, &series.slug);
		Ok(series.id.to_string())
	}

	fn search_by_title(&self, query: &str, page: i32) -> Result<MangaPageResult> {
		if self.series.borrow().is_none() {
			self.refresh_series()?;
		}
		let series = self.series.borrow();
		let Some(series) = series.as_deref() else {
			bail!("No se pudo cargar la lista de series");
		};

		let query = query.to_lowercase();
		let matches: Vec<&MangaListItemDto> = series
			.iter()
			.filter(|item| item.name.to_lowercase().contains(&query))
			.collect();

		let start = (page.max(1) as usize - 1) * SEARCH_PAGE_SIZE;
		Ok(MangaPageResult {
			entries: matches
				.iter()
				.skip(start)
				.take(SEARCH_PAGE_SIZE)
				.map(|item| item.to_manga())
				.collect(),
			has_next_page: start + SEARCH_PAGE_SIZE < matches.len(),
		})
	}
}

impl ListingProvider for OlympusScanlation {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let base_url = self.base_url();
		let url = match listing.id.as_str() {
			"Popular" => rankings_url(&base_url, "total_ranking", page),
			"Latest" => latest_url(&base_url, page),
			_ => bail!("Listado no válido: {}", listing.id),
		};
		let response = get_json::<Paginated<MangaListItemDto>>(&url)?;
		self.remember_slugs(&response.data);
		Ok(response.into())
	}
}

impl Home for OlympusScanlation {
	fn get_home(&self) -> Result<HomeLayout> {
		// Send the empty sections first so the app can show them right away, then
		// fill in each one as its request finishes.
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Tendencias del mes".into()),
					subtitle: None,
					value: HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("Últimas actualizaciones".into()),
					subtitle: None,
					value: HomeComponentValue::empty_manga_chapter_list(),
				},
				HomeComponent {
					title: Some("Populares".into()),
					subtitle: None,
					value: HomeComponentValue::empty_scroller(),
				},
			],
		}));

		let base_url = self.base_url();

		let trending = get_json::<Paginated<MangaListItemDto>>(&rankings_url(
			&base_url,
			"monthly_ranking",
			1,
		))?;
		self.remember_slugs(&trending.data);
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Tendencias del mes".into()),
			subtitle: None,
			value: HomeComponentValue::MangaList {
				ranking: true,
				page_size: Some(5),
				entries: manga_links(trending.data),
				listing: None,
			},
		}));

		let latest = get_json::<Paginated<LatestItemDto>>(&latest_url(&base_url, 1))?;
		let entries = latest
			.data
			.into_iter()
			.filter(|item| item.kind.as_deref() == Some("comic"))
			.filter_map(|item| {
				self.remember_slug(item.id, &item.slug);
				// The newest chapter comes first.
				let chapter = item.last_chapters.into_iter().next()?;
				Some(MangaWithChapter {
					chapter: chapter.into_chapter(&base_url, &item.slug),
					manga: Manga {
						key: item.id.to_string(),
						title: item.name,
						cover: item.cover,
						..Default::default()
					},
				})
			})
			.collect();
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Últimas actualizaciones".into()),
			subtitle: None,
			value: HomeComponentValue::MangaChapterList {
				page_size: Some(4),
				entries,
				listing: Some(Listing {
					id: "Latest".into(),
					name: "Últimas actualizaciones".into(),
					..Default::default()
				}),
			},
		}));

		let popular =
			get_json::<Paginated<MangaListItemDto>>(&rankings_url(&base_url, "total_ranking", 1))?;
		self.remember_slugs(&popular.data);
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Populares".into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: manga_links(popular.data),
				listing: Some(Listing {
					id: "Popular".into(),
					name: "Populares".into(),
					..Default::default()
				}),
			},
		}));

		// Everything was already sent as partial results.
		Ok(HomeLayout::default())
	}
}

impl DeepLinkHandler for OlympusScanlation {
	/// Handles, on any domain:
	///   /series/comic-{slug}[/capitulo/{number}]  -> the series
	///   /capitulo/{chapter id}/comic-{slug}       -> the chapter
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let url = url.split(['?', '#']).next().unwrap_or_default();
		let Some((_, path)) = url
			.split_once("://")
			.and_then(|(_, rest)| rest.split_once('/'))
		else {
			return Ok(None);
		};
		let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();

		match parts.as_slice() {
			// The /capitulo/{number} form only has the chapter number, not its id, so
			// it opens the series.
			["series", series, ..] => {
				let Some(slug) = series.strip_prefix("comic-") else {
					return Ok(None);
				};
				Ok(Some(DeepLinkResult::Manga {
					key: self.key_for_slug(slug)?,
				}))
			}
			["capitulo", chapter_id, series] => {
				let Some(slug) = series.strip_prefix("comic-") else {
					return Ok(None);
				};
				Ok(Some(DeepLinkResult::Chapter {
					manga_key: self.key_for_slug(slug)?,
					key: (*chapter_id).into(),
				}))
			}
			_ => Ok(None),
		}
	}
}

impl BaseUrlProvider for OlympusScanlation {
	fn get_base_url(&self) -> Result<String> {
		Ok(self.base_url())
	}
}

register_source!(
	OlympusScanlation,
	ListingProvider,
	Home,
	DeepLinkHandler,
	BaseUrlProvider
);
