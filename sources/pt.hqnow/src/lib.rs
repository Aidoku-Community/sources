#![no_std]
extern crate alloc;

mod graphql;
mod helpers;
mod models;

use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeComponentValue, HomeLayout, Link, Listing, ListingKind, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source, Viewer,
	alloc::{String, Vec, format, vec},
	prelude::*,
};
use alloc::collections::BTreeMap;
use core::cell::RefCell;

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
				if manga.cover.is_none()
					&& let Ok(id) = manga.key.parse::<i32>()
					&& let Some(url) = cache.get(&id)
				{
					manga.cover = Some(url.clone());
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
		if query.is_none() {
			for filter in &filters {
				match filter {
					FilterValue::Sort { index: 1, .. } => order_by_views = true,
					FilterValue::Select { id, value } if id == "publisher" && !value.is_empty() => {
						publisher_id = value.parse::<i32>().ok();
					}
					_ => {}
				}
			}
		}

		let hqs = if let Some(q) = query {
			helpers::execute_query::<models::ByNameResponse>(
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
			helpers::execute_query::<models::ByFiltersResponse>(
				&graphql::GraphQLQuery::HQS_BY_FILTERS,
				Some(vars),
			)?
			.get_hqs_by_filters
		} else {
			helpers::execute_query::<models::RecentResponse>(
				&graphql::GraphQLQuery::RECENTLY_UPDATED,
				None,
			)?
			.get_recently_updated_hqs
		};

		let (entries, has_next_page) = helpers::paginate(hqs, page, PER_PAGE);
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
		let id: i32 = manga.key.parse().map_err(|_| error!("invalid manga key"))?;

		let detail = helpers::execute_query::<models::ByIdResponse>(
			&graphql::GraphQLQuery::HQS_BY_ID,
			Some(serde_json::json!({ "id": id })),
		)?
		.get_hqs_by_id
		.into_iter()
		.next()
		.ok_or_else(|| error!("manga not found"))?;

		if needs_details {
			// format! borrows detail.name, so the move below is still valid
			manga.url = Some(format!(
				"https://www.hq-now.com/hq/{}/{}",
				detail.id, detail.name
			));
			manga.title = detail.name;
			manga.cover = helpers::to_https(detail.hq_cover);
			manga.description = detail.synopsis;
			manga.status = helpers::parse_status(detail.status.as_deref());
			manga.authors = detail.publisher_name.map(|p| vec![p]);
			manga.content_rating = ContentRating::Safe;
			manga.viewer = Viewer::LeftToRight;
			// Populate cache so future search results can show this cover.
			if let Some(url) = &manga.cover {
				self.cover_cache.borrow_mut().insert(id, url.clone());
			}
		}

		if needs_chapters {
			manga.chapters = Some(detail.capitulos.into_iter().map(Into::into).collect());
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_id: i32 = chapter
			.key
			.parse()
			.map_err(|_| error!("invalid chapter key"))?;

		let pictures = helpers::execute_query::<models::ChapterByIdResponse>(
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
					helpers::to_https(Some(p.picture_url)).unwrap_or_default(),
				),
				..Default::default()
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
				helpers::execute_query::<models::ByFiltersResponse>(
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
				helpers::execute_query::<models::RecentResponse>(
					&graphql::GraphQLQuery::RECENTLY_UPDATED,
					None,
				)?
				.get_recently_updated_hqs
			}
		};

		let (entries, has_next_page) = helpers::paginate(hqs, page, PER_PAGE);
		Ok(MangaPageResult {
			entries: self.hqs_to_mangas(entries),
			has_next_page,
		})
	}
}

// ── Home ──────────────────────────────────────────────────────────────────────

impl Home for HQnow {
	fn get_home(&self) -> Result<HomeLayout> {
		let carousel = helpers::execute_query::<models::CarouselResponse>(
			&graphql::GraphQLQuery::CAROUSEL,
			None,
		)?
		.get_carousel_of_hqs
		.into_iter()
		.map(models::CarouselItem::into_manga)
		.collect::<Vec<_>>();

		let recent = self.hqs_to_mangas(
			helpers::execute_query::<models::RecentResponse>(
				&graphql::GraphQLQuery::RECENTLY_UPDATED,
				None,
			)?
			.get_recently_updated_hqs
			.into_iter()
			.take(20)
			.collect(),
		);

		let popular = self.hqs_to_mangas(
			helpers::execute_query::<models::ByFiltersResponse>(
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
