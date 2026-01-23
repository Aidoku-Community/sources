use crate::models::*;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, Listing, ListingKind, ListingProvider, Manga, MangaPageResult, Page, PageContent,
	Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	prelude::*,
};

const PER_PAGE: i32 = 18;
const BASE_URL: &str = "https://manhwaweb.com";
const BACKEND_URL: &str = "https://manhwawebbackend-production.up.railway.app";

pub struct ManhwaWeb;

impl Source for ManhwaWeb {
	fn new() -> Self {
		crate::helpers::setup_rate_limit();
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// API is 0-based, Aidoku is 1-based.
		let api_page = page - 1;
		let mut url = format!(
			"{BACKEND_URL}/manhwa/library?page={}&perPage={}",
			api_page, PER_PAGE
		);
		let mut qs = QueryParameters::new();

		// Handle search query
		if let Some(q) = query {
			let trimmed = q.trim();
			if !trimmed.is_empty() {
				qs.push("buscar", Some(trimmed));
			}
		}

		let mut erotic_filter_set = false;
		let mut genre_values: Vec<String> = Vec::new();

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => {
					if !value.is_empty() {
						// Pass ID directly as API expects (e.g., 'tipo', 'demografia')
						qs.push(&id, Some(&value));

						if id == "erotico" {
							erotic_filter_set = true;
						}
					}
				}
				FilterValue::MultiSelect { id, included, .. } => {
					// For genres
					if id == "genres" || id == "genreIds" || id == "generes" {
						for val in included {
							genre_values.push(val);
						}
					}
				}
				FilterValue::Sort { index, .. } => {
					let sort_val = match index {
						0 => "alfabetico",
						2 => "num_chapter",
						_ => "creacion", // Default to creation date
					};
					qs.push("order_item", Some(sort_val));
				}
				_ => {}
			}
		}

		// Handle Genres: joined by 'a' (e.g., "1a2a3")
		if !genre_values.is_empty() {
			let joined_genres = genre_values.join("a");
			qs.push("generes", Some(&joined_genres));
		}

		// Default to "no" erotic content if the filter wasn't explicitly set
		if !erotic_filter_set {
			qs.push("erotico", Some("no"));
		}

		// Ensure qs is appended with '&' prefix because base URL might have query params already.
		if !qs.is_empty() {
			url.push_str(&format!("&{}", qs));
		}

		let data = crate::helpers::create_request(&url, "GET")?
			.header("Referer", &format!("{}/", BASE_URL))
			.json_owned::<LibraryResponse>()?;
		let entries = data
			.data
			.into_iter()
			.map(|m| m.to_manga(BASE_URL))
			.collect();
		// Pagination check: API returns 'next': boolean.
		let has_next_page = data.next;

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BACKEND_URL}/manhwa/see/{}", manga.key);
		let data = crate::helpers::create_request(&url, "GET")?
			.header("Referer", &format!("{}/", BASE_URL))
			.json_owned::<SeeResponse>()?;

		if needs_details {
			manga.copy_from(data.parse_manga(BASE_URL));
		}

		if needs_chapters {
			manga.chapters = Some(data.parse_chapters(BASE_URL));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BACKEND_URL}/chapters/see/{}", chapter.key);
		let data =
			crate::helpers::create_request(&url, "GET")?.json_owned::<ChapterSeeResponse>()?;

		Ok(data
			.chapter
			.img
			.into_iter()
			.enumerate()
			.map(|(_i, url)| Page {
				content: PageContent::Url(url, None),
				..Default::default()
			})
			.collect())
	}
}

impl DeepLinkHandler for ManhwaWeb {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(path) = url.strip_prefix(BASE_URL) {
			if path.starts_with("/manhwa/") {
				let id = path.trim_start_matches("/manhwa/");
				return Ok(Some(DeepLinkResult::Manga { key: id.into() }));
			}
		}
		Ok(None)
	}
}

impl Home for ManhwaWeb {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut components = Vec::new();

		// 1. Hero: Latest Chapters ("Nuevos Capitulos") - BigScroller
		let url_latest = format!("{BACKEND_URL}/manhwa/nuevos");
		if let Ok(data) = crate::helpers::create_request(&url_latest, "GET")
			.and_then(|r| r.json_owned::<NuevosResponse>())
		{
			let latest_entries: Vec<Manga> = data
				.manhwas
				.spanish_manhwas
				.into_iter()
				.map(|m| {
					let group = m.gru_name.unwrap_or_default();
					let subtitle = format!("Cap. {} - {}", m.chapter, group);
					let url = Some(format!("{}/manhwa/{}", BASE_URL, m.id_manhwa));

					Manga {
						key: m.id_manhwa.into(),
						title: m.name_manhwa,
						authors: Some(vec![subtitle]),
						cover: m.img.map(|s| s.into()),
						url,
						..Default::default()
					}
				})
				.collect();

			components.push(HomeComponent {
				title: Some("Nuevos Capitulos".into()),
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: latest_entries,
					auto_scroll_interval: Some(5.0),
				},
			});
		}

		// 2. New Works ("Nuevas Obras") - Scroller
		let url_new = format!(
			"{BACKEND_URL}/manhwa/library?page=0&perPage=12&order_item=creacion&order_dir=desc"
		);
		if let Ok(data) = crate::helpers::create_request(&url_new, "GET")
			.and_then(|r| r.json_owned::<LibraryResponse>())
		{
			let entries: Vec<aidoku::Link> = data
				.data
				.into_iter()
				.filter(|m| m.erotic.as_deref() != Some("si"))
				.map(|m| {
					let categories = m.categories.clone();
					let mut manga = m.to_manga(BASE_URL);
					if let Some(cats) = categories {
						let tags: Vec<String> = cats
							.iter()
							.map(|id| crate::helpers::get_genre_name(&id.to_string()))
							.collect();
						manga.tags = Some(tags);
					}
					manga.into()
				})
				.collect();

			components.push(HomeComponent {
				title: Some("Nuevas Obras".into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries,
					listing: Some(Listing {
						id: "New".into(),
						name: "Nuevas Obras".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		Ok(HomeLayout { components })
	}
}

impl ListingProvider for ManhwaWeb {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let name = listing.id.as_str();
		let api_page = page - 1;

		if name.starts_with("Genre:") {
			let id = name.split(':').nth(1).unwrap_or("");
			// Ensure 'creacion' (creation date) sort is properly applied for these listings too
			let url = format!(
				"{BACKEND_URL}/manhwa/library?page={}&perPage={}&erotico=no&generes={}&order_item=creacion&order_dir=desc",
				api_page, PER_PAGE, id
			);
			let resp =
				crate::helpers::create_request(&url, "GET")?.json_owned::<LibraryResponse>()?;
			let entries: Vec<Manga> = resp
				.data
				.into_iter()
				.map(|m| m.to_manga(BASE_URL))
				.collect();
			return Ok(MangaPageResult {
				entries,
				has_next_page: resp.next,
			});
		}

		match name {
			// "Latest" and "Popular" standard tabs fallback
			// Note: The API does not strictly support a "Popular" endpoint that differs significantly from "Nuevos" in this context without specific implementation.
			"Latest" | "Popular" => {
				if page > 1 {
					return Ok(MangaPageResult {
						entries: vec![],
						has_next_page: false,
					});
				}
				let url = format!("{BACKEND_URL}/manhwa/nuevos");
				let resp =
					crate::helpers::create_request(&url, "GET")?.json_owned::<NuevosResponse>()?;
				let entries: Vec<Manga> = resp
					.manhwas
					.spanish_manhwas
					.into_iter()
					.map(|m| {
						let url = Some(format!("{}/manhwa/{}", BASE_URL, m.id_manhwa));
						Manga {
							key: m.id_manhwa.into(),
							title: m.name_manhwa,
							cover: m.img.map(|s| s.into()),
							url,
							..Default::default()
						}
					})
					.collect();
				Ok(MangaPageResult {
					entries,
					has_next_page: false,
				})
			}
			"New" => self.get_search_manga_list(
				None,
				page,
				vec![FilterValue::Sort {
					index: 3,
					ascending: false,
					id: "sortBy".into(),
				}],
			),
			"Erotic" | "+18 (Erotic)" => {
				let url = format!(
					"{BACKEND_URL}/manhwa/library?page={}&perPage={}&erotico=si",
					api_page, PER_PAGE
				);
				let data = crate::helpers::create_request(&url, "GET")?
					.header("Referer", &format!("{}/", BASE_URL))
					.json_owned::<LibraryResponse>()?;
				let entries = data
					.data
					.into_iter()
					.map(|m| m.to_manga(BASE_URL))
					.collect();
				Ok(MangaPageResult {
					entries,
					has_next_page: data.next,
				})
			}
			// Search fallback
			_ => {
				let filters = vec![];
				self.get_search_manga_list(None, page, filters)
			}
		}
	}
}
