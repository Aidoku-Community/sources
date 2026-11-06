#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, HomePartialResult, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::{QueryParameters, encode_uri_component},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;
mod models;
use helpers::{ACCEPT, get_json, last_segment, listing};
use models::{DetailsDto, LatestResponse, LibraryResponse, NuevosResponse, PagesResponse};

pub const BASE_URL: &str = "https://manhwaweb.com";
pub const API_URL: &str = "https://api.manhwaweb.com";

/// `order_item` values, in the same order as the sort options in filters.json.
const SORT_OPTIONS: [&str; 3] = ["alfabetico", "creacion", "num_chapter"];

struct ManhwaWeb;

impl Source for ManhwaWeb {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut params = QueryParameters::new();
		params.push(
			"buscar",
			Some(query.as_deref().map(str::trim).unwrap_or_default()),
		);

		let mut sort = ("creacion", "desc");
		for filter in filters {
			match filter {
				// Tipo, Demografía, Estado and Erótico. Their ids in filters.json are the
				// query parameter names. An empty value means "Ver todo".
				FilterValue::Select { id, value } if !value.is_empty() => {
					params.push(&id, Some(&value));
				}
				FilterValue::MultiSelect { id, included, .. } if !included.is_empty() => {
					params.push(&id, Some(&included.join("a")));
				}
				FilterValue::Sort {
					index, ascending, ..
				} => {
					let item = SORT_OPTIONS
						.get(index as usize)
						.copied()
						.unwrap_or("creacion");
					sort = (item, if ascending { "asc" } else { "desc" });
				}
				_ => {}
			}
		}
		params.push("order_item", Some(sort.0));
		params.push("order_dir", Some(sort.1));
		// The API counts pages from 0.
		params.push("page", Some(&(page - 1).to_string()));

		let response = get_json::<LibraryResponse>(&format!("/manhwa/library?{params}"))?;
		Ok(MangaPageResult {
			entries: response
				.data
				.into_iter()
				.map(|item| item.into_manga())
				.collect(),
			has_next_page: response.next,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let details =
			get_json::<DetailsDto>(&format!("/manhwa/see/{}", encode_uri_component(&manga.key)))?;

		if needs_details {
			details.update_manga(&mut manga);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(details.chapters());
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let response = get_json::<PagesResponse>(&format!(
			"/chapters/see/{}",
			encode_uri_component(&chapter.key)
		))?;
		Ok(response
			.chapter
			.img
			.into_iter()
			.filter(|url| url.starts_with("http"))
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for ManhwaWeb {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		// Neither endpoint has more than one page.
		if page > 1 {
			return Ok(MangaPageResult::default());
		}

		let entries = match listing.id.as_str() {
			"Popular" => get_json::<NuevosResponse>("/manhwa/nuevos")?
				.top
				.into_mangas(),
			"Latest" => get_json::<LatestResponse>("/latest/new-manhwa")?
				.manhwas
				.into_items()
				.into_iter()
				.map(|item| item.into_manga())
				.collect(),
			_ => bail!("Listado no válido: {}", listing.id),
		};
		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}
}

impl Home for ManhwaWeb {
	fn get_home(&self) -> Result<HomeLayout> {
		// Send the empty sections first so the app can show them right away, then
		// fill them in as each request finishes.
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Populares".into()),
					subtitle: None,
					value: HomeComponentValue::empty_manga_list(),
				},
				HomeComponent {
					title: Some("Últimas actualizaciones".into()),
					subtitle: None,
					value: HomeComponentValue::empty_manga_chapter_list(),
				},
				HomeComponent {
					title: Some("Nuevas obras".into()),
					subtitle: None,
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Nuevas obras +18".into()),
					subtitle: None,
					value: HomeComponentValue::empty_scroller(),
				},
			],
		}));

		let nuevos = get_json::<NuevosResponse>("/manhwa/nuevos")?;
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Populares".into()),
			subtitle: None,
			value: HomeComponentValue::MangaList {
				ranking: true,
				page_size: Some(5),
				entries: nuevos
					.top
					.into_mangas()
					.into_iter()
					.map(Into::into)
					.collect(),
				listing: Some(listing("Popular", "Populares")),
			},
		}));
		for (title, items) in [
			("Nuevas obras", nuevos.new_series),
			("Nuevas obras +18", nuevos.new_series_18),
		] {
			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(title.into()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: items
						.into_iter()
						.map(|item| item.into_manga().into())
						.collect(),
					listing: None,
				},
			}));
		}

		let latest = get_json::<LatestResponse>("/latest/new-manhwa")?;
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Últimas actualizaciones".into()),
			subtitle: None,
			value: HomeComponentValue::MangaChapterList {
				page_size: None,
				entries: latest
					.manhwas
					.into_items()
					.into_iter()
					.map(|item| item.into_manga_with_chapter())
					.collect(),
				listing: Some(listing("Latest", "Últimas actualizaciones")),
			},
		}));

		Ok(HomeLayout::default())
	}
}

impl ImageRequestProvider for ManhwaWeb {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Accept", ACCEPT)
			.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for ManhwaWeb {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.or_else(|| url.strip_prefix("https://www.manhwaweb.com"))
		else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();

		// `/manhwa/{slug}` (also `/manga/{slug}` in older links)
		if path.starts_with("/manhwa/") || path.starts_with("/manga/") {
			return Ok(Some(DeepLinkResult::Manga {
				key: last_segment(path).into(),
			}));
		}

		// `/leer/{slug}-{number}_{version}`
		if path.starts_with("/leer/") {
			let key = last_segment(path);
			if let Some((manga_key, _)) = key.rsplit_once('-') {
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: key.into(),
				}));
			}
		}

		Ok(None)
	}
}

register_source!(
	ManhwaWeb,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
