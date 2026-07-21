#![no_std]
extern crate alloc;

mod graphql;
mod models;

use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, ListingKind, ListingProvider,
	Manga, MangaPageResult, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, format, vec},
	imports::net::Request,
	prelude::*,
};
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use core::cell::RefCell;
use serde::de::DeserializeOwned;

const GRAPHQL_URL: &str = "https://admin.hq-now.com/graphql";

// Maps publisher display name (as it appears in filters.json) to editoraId.
const PUBLISHERS: &[(&str, i32)] = &[
	("AfterShock Comics", 45),
	("Alterna Comics", 52),
	("Amigo Comics", 50),
	("Archie Comics", 7),
	("Avatar Press", 19),
	("Beckett Comics", 43),
	("Black Mask Studios", 51),
	("Boom Studios", 12),
	("Capcom", 41),
	("Chaos Comics", 26),
	("Dargaud", 4),
	("Dark Horse Comics", 6),
	("DC Comics", 1),
	("Delcourt", 30),
	("Dell Comics", 24),
	("Desconhecida", 16),
	("Devir Livraria", 29),
	("Di\u{e1}bolo Ediciones", 27),
	("Difus\u{e3}o Verbo", 42),
	("Dupuis", 39),
	("Dynamite Entertainment", 20),
	("Editora 12 bis", 34),
	("Europe comics", 56),
	("Gl\u{e9}nat", 21),
	("Graphix", 55),
	("Harris Publications", 17),
	("Humanoids", 57),
	("Icon", 14),
	("IDW Publishing", 8),
	("Image Comics", 5),
	("Kingdom Comics", 53),
	("L&PM Editores", 40),
	("Le Lombard", 37),
	("Marvel Comics", 3),
	("Merib\u{e9}rica", 18),
	("Monkeybrain comics", 49),
	("Norma Editorial", 15),
	("Oni Press", 22),
	("Radical Comics", 10),
	("Rue de S\u{e8}vres", 38),
	("Soleil", 36),
	("Titan Comics", 46),
	("Udon Comics", 25),
	("Valiant Comics", 48),
	("Vents d'Ouest", 32),
	("Vertigo", 13),
	("Virgin Comics", 47),
	("WildStorm", 2),
];

// ── GraphQL helper ────────────────────────────────────────────────────────────

fn execute_query<T: DeserializeOwned>(
	gql: &graphql::GraphQLQuery,
	variables: Option<serde_json::Value>,
) -> Result<T> {
	let mut body = serde_json::json!({
		"operationName": gql.operation_name,
		"query": gql.query,
	});
	if let Some(vars) = variables {
		body["variables"] = vars;
	}
	let body_str = body.to_string();
	let resp = Request::post(GRAPHQL_URL)
		.map_err(|_| AidokuError::message("network error"))?
		.header("Content-Type", "application/json")
		.body(body_str.as_bytes())
		.string()?;
	let wrapper: models::GqlResponse<T> =
		serde_json::from_str(&resp).map_err(|_| AidokuError::message("parse error"))?;
	wrapper.data.ok_or_else(|| AidokuError::message("no data"))
}

// ── Pagination helper ─────────────────────────────────────────────────────────

/// Slices a locally-held Vec into one page of results. Used because most
/// HQ-Now endpoints return all results in a single response.
fn paginate<T>(items: Vec<T>, page: i32, per_page: usize) -> (Vec<T>, bool) {
	let start = ((page - 1) as usize) * per_page;
	let has_next = start + per_page < items.len();
	let slice = items.into_iter().skip(start).take(per_page).collect();
	(slice, has_next)
}

// ── Source ────────────────────────────────────────────────────────────────────

struct HQnow {
	/// Maps manga id → cover URL, kept consistent across all listing methods.
	cover_cache: RefCell<BTreeMap<i32, String>>,
}

impl HQnow {
	/// Converts a batch of `HqBasic` entries to `Manga`, keeping the cover
	/// cache consistent in both directions:
	/// - entries **with** covers (listings, home) populate the cache
	/// - entries **without** covers (search results) pull from it
	fn hqs_to_mangas(&self, hqs: Vec<models::HqBasic>) -> Vec<Manga> {
		let mangas: Vec<Manga> = hqs.into_iter().map(models::HqBasic::into_manga).collect();
		{
			let mut cache = self.cover_cache.borrow_mut();
			for manga in &mangas {
				if let (Ok(id), Some(url)) = (manga.key.parse::<i32>(), &manga.cover) {
					cache.insert(id, url.clone());
				}
			}
		}
		let cache = self.cover_cache.borrow();
		mangas
			.into_iter()
			.map(|mut manga| {
				if manga.cover.is_none() {
					if let Ok(id) = manga.key.parse::<i32>() {
						if let Some(url) = cache.get(&id) {
							manga.cover = Some(url.clone());
						}
					}
				}
				manga
			})
			.collect()
	}
}

impl Source for HQnow {
	fn new() -> Self {
		Self {
			cover_cache: RefCell::new(BTreeMap::new()),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		const PER_PAGE: usize = 20;

		let mut order_by_views = false;
		let mut publisher_id: Option<i32> = None;
		for filter in &filters {
			match filter {
				FilterValue::Sort { index: 1, .. } => order_by_views = true,
				FilterValue::Select { id, value } if id == "publisher" && value != "Todas" => {
					if let Some(&(_, pid)) =
						PUBLISHERS.iter().find(|&&(name, _)| name == value.as_str())
					{
						publisher_id = Some(pid);
					}
				}
				_ => {}
			}
		}

		let hqs = if let Some(q) = query {
			execute_query::<models::ByNameResponse>(
				&graphql::GraphQLQuery::HQS_BY_NAME,
				Some(serde_json::json!({ "name": q })),
			)?
			.get_hqs_by_name
		} else if order_by_views || publisher_id.is_some() {
			let mut vars = serde_json::json!({ "loadCovers": true });
			if order_by_views {
				vars["orderByViews"] = serde_json::Value::Bool(true);
			}
			if let Some(pid) = publisher_id {
				vars["publisherId"] = serde_json::json!(pid);
			}
			execute_query::<models::ByFiltersResponse>(
				&graphql::GraphQLQuery::HQS_BY_FILTERS,
				Some(vars),
			)?
			.get_hqs_by_filters
		} else {
			execute_query::<models::RecentResponse>(&graphql::GraphQLQuery::RECENTLY_UPDATED, None)?
				.get_recently_updated_hqs
		};

		let (entries, has_next_page) = paginate(hqs, page, PER_PAGE);
		Ok(MangaPageResult {
			entries: self.hqs_to_mangas(entries),
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if !needs_details && !needs_chapters {
			return Ok(manga);
		}

		let id: i32 = manga
			.key
			.parse()
			.map_err(|_| AidokuError::message("invalid manga key"))?;

		let detail = execute_query::<models::ByIdResponse>(
			&graphql::GraphQLQuery::HQS_BY_ID,
			Some(serde_json::json!({ "id": id })),
		)?
		.get_hqs_by_id
		.into_iter()
		.next()
		.ok_or_else(|| AidokuError::message("manga not found"))?;

		if needs_details {
			// format! borrows detail.name, so the move below is still valid
			manga.url = Some(format!(
				"https://www.hq-now.com/hq/{}/{}",
				detail.id, detail.name
			));
			manga.title = detail.name;
			manga.cover = models::to_https(detail.hq_cover);
			manga.description = detail.synopsis;
			manga.status = models::parse_status(detail.status.as_deref());
			manga.authors = detail.publisher_name.map(|p| vec![p]);
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::LeftToRight;
			// Populate cache so future search results can show this cover.
			if let Some(url) = &manga.cover {
				self.cover_cache.borrow_mut().insert(id, url.clone());
			}
		}

		if needs_chapters {
			manga.chapters = Some(
				detail
					.capitulos
					.into_iter()
					.map(models::HqChapter::into_chapter)
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_id: i32 = chapter
			.key
			.parse()
			.map_err(|_| AidokuError::message("invalid chapter key"))?;

		let pictures = execute_query::<models::ChapterByIdResponse>(
			&graphql::GraphQLQuery::CHAPTER_BY_ID,
			Some(serde_json::json!({ "chapterId": chapter_id })),
		)?
		.get_chapter_by_id
		.map(|c| c.pictures)
		.unwrap_or_default();

		Ok(pictures
			.into_iter()
			.map(|p| Page {
				content: PageContent::url(
					models::to_https(Some(p.picture_url)).unwrap_or_default(),
				),
				thumbnail: None,
				has_description: false,
				description: None,
			})
			.collect())
	}
}

// ── ListingProvider ───────────────────────────────────────────────────────────

impl ListingProvider for HQnow {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		const PER_PAGE: usize = 20;

		let hqs: Vec<models::HqBasic> = match listing.id.as_str() {
			"popular" => {
				execute_query::<models::ByFiltersResponse>(
					&graphql::GraphQLQuery::HQS_BY_FILTERS,
					Some(serde_json::json!({
						"orderByViews": true,
						"limit": 300,
						"loadCovers": true,
					})),
				)?
				.get_hqs_by_filters
			}
			_ => {
				execute_query::<models::RecentResponse>(
					&graphql::GraphQLQuery::RECENTLY_UPDATED,
					None,
				)?
				.get_recently_updated_hqs
			}
		};

		let (entries, has_next_page) = paginate(hqs, page, PER_PAGE);
		Ok(MangaPageResult {
			entries: self.hqs_to_mangas(entries),
			has_next_page,
		})
	}
}

// ── Home ──────────────────────────────────────────────────────────────────────

impl Home for HQnow {
	fn get_home(&self) -> Result<HomeLayout> {
		let carousel =
			execute_query::<models::CarouselResponse>(&graphql::GraphQLQuery::CAROUSEL, None)?
				.get_carousel_of_hqs
				.into_iter()
				.map(models::CarouselItem::into_manga)
				.collect::<Vec<_>>();

		let recent = self.hqs_to_mangas(
			execute_query::<models::RecentResponse>(
				&graphql::GraphQLQuery::RECENTLY_UPDATED,
				None,
			)?
			.get_recently_updated_hqs
			.into_iter()
			.take(20)
			.collect(),
		);

		let popular = self.hqs_to_mangas(
			execute_query::<models::ByFiltersResponse>(
				&graphql::GraphQLQuery::HQS_BY_FILTERS,
				Some(serde_json::json!({
					"orderByViews": true,
					"limit": 20,
					"loadCovers": true,
				})),
			)?
			.get_hqs_by_filters,
		);

		let mut components = Vec::new();

		if !carousel.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("Destaques")),
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: carousel,
					auto_scroll_interval: Some(5.0),
				},
			});
		}

		if !recent.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("Atualizados Recentemente")),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: recent.into_iter().map(Link::from).collect(),
					listing: Some(Listing {
						id: String::from("recent"),
						name: String::from("Atualizados Recentemente"),
						kind: ListingKind::List,
					}),
				},
			});
		}

		if !popular.is_empty() {
			components.push(HomeComponent {
				title: Some(String::from("Mais Vistos")),
				subtitle: None,
				value: HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(10),
					entries: popular.into_iter().map(Link::from).collect(),
					listing: Some(Listing {
						id: String::from("popular"),
						name: String::from("Mais Vistos"),
						kind: ListingKind::List,
					}),
				},
			});
		}

		Ok(HomeLayout { components })
	}
}

// ── DeepLinkHandler ───────────────────────────────────────────────────────────

impl DeepLinkHandler for HQnow {
	/// Handles URLs in these patterns:
	/// - `https://www.hq-now.com/hq/{manga_id}/{slug}`
	/// - `https://www.hq-now.com/hq-reader/{manga_id}/{slug}/chapter/{chapter_id}/page/{n}`
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.find("hq-now.com")
			.map(|i| &url[i + "hq-now.com".len()..])
			.unwrap_or(&url);

		let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

		match parts.first().copied() {
			Some("hq-reader") if parts.len() >= 5 => {
				let manga_key = parts.get(1).copied().unwrap_or("");
				if let Some(ch_idx) = parts.iter().position(|&s| s == "chapter")
					&& let Some(chapter_id) = parts.get(ch_idx + 1).copied()
				{
					return Ok(Some(DeepLinkResult::Chapter {
						manga_key: String::from(manga_key),
						key: String::from(chapter_id),
					}));
				}
				Ok(None)
			}
			Some("hq") if parts.len() >= 2 => {
				let manga_key = parts.get(1).copied().unwrap_or("");
				Ok(Some(DeepLinkResult::Manga {
					key: String::from(manga_key),
				}))
			}
			_ => Ok(None),
		}
	}
}

register_source!(HQnow, ListingProvider, Home, DeepLinkHandler);
