#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, Home,
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, ImageRequestProvider,
	Listing, ListingKind, ListingProvider, Manga, MangaPageResult, MultiSelectFilter, Page,
	PageContent, PageContext, Result, Source,
	alloc::{format, string::String, vec, vec::Vec},
	imports::{
		net::{Request, TimeUnit, set_rate_limit},
		std::send_partial_result,
	},
	prelude::*,
};

mod helpers;
mod models;
use helpers::*;
use models::*;

struct Kagane;

impl Source for Kagane {
	fn new() -> Self {
		set_rate_limit(3, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut sort_idx = 0usize;
		let mut ascending = false;
		let mut search = SearchFilters::default();

		for filter in filters {
			match filter {
				FilterValue::Sort {
					index,
					ascending: asc,
					..
				} => {
					sort_idx = index as usize;
					ascending = asc;
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"genre" => {
						search.genres_included = included;
						search.genres_excluded = excluded;
					}
					"status" => search.upload_statuses = included,
					"publication_status" => search.publication_statuses = included,
					"format" => search.formats = included,
					"ratings" => search.ratings = included,
					"sources" => search.sources = included,
					_ => {}
				},
				FilterValue::Select { id, value } => match id.as_str() {
					"genre_mode" => search.genres_match_any = value == "any",
					"tag_mode" => search.tags_match_any = value == "any",
					"disturbing" => {
						search.include_disturbing = match value.as_str() {
							"include" => Some(true),
							"exclude" => Some(false),
							_ => None,
						}
					}
					_ => {}
				},
				FilterValue::Text { id, value } => {
					let value = value.trim();
					if value.is_empty() {
						continue;
					}
					match id.as_str() {
						"tags" => (search.tags_included, search.tags_excluded) = tag_ids(value)?,
						"author" => search.staff = Some(staff_id(value)?),
						"group" => search.group = Some(group_id(value)?),
						_ => {}
					}
				}
				FilterValue::Range { id, from, to } => search.ranges.push((id, from, to)),
				_ => {}
			}
		}

		let query = query.filter(|query| !query.is_empty());
		// Relevance means nothing without a title, so browsing falls back to the most viewed.
		let sort = match SORT_PARAMS.get(sort_idx).copied().unwrap_or_default() {
			"" if query.is_none() => "total_views",
			sort => sort,
		};
		let url = search_url(page, 35, sort, ascending);
		let body = build_search_body(query.as_deref(), search)?;
		search_page(&url, body)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// Chapters are credited to the series' source, so they need its details and the list
		// of sources too.
		let mut requests = vec![api_get(&format!("{}/series/{}", api_base(), manga.key))?];
		if needs_chapters {
			requests.push(api_get(&format!(
				"{}/series/{}/books/index?fields=List",
				api_base(),
				manga.key
			))?);
		}
		let show_source = show_source();
		if needs_chapters || show_source {
			requests.push(sources_request()?);
		}
		let mut responses = Request::send_all(requests).into_iter();
		let mut next = || {
			responses
				.next()
				.ok_or_else(|| error!("Kagane sent no response"))
		};
		let det: SeriesDetail = next()??.get_json_owned()?;
		let index: Option<BookIndex> = if needs_chapters {
			Some(next()??.get_json_owned()?)
		} else {
			None
		};
		let sources = parse_sources(responses.next());

		if needs_details {
			// The sources may be here only to credit chapters.
			let title_sources = if show_source { sources.as_slice() } else { &[] };
			manga.title = source_title(&det.title, title_sources, det.source_id.as_deref());
			manga.cover = det
				.series_covers
				.first()
				.map(|c| format!("{}/image/{}", api_base(), c.image_id));
			manga.url = Some(format!("{}/series/{}", base_url(), manga.key));
			manga.status = parse_status(&det.upload_status);
			manga.viewer = parse_viewer(det.format.as_deref());
			manga.content_rating = parse_content_rating(
				det.age_rating.as_deref(),
				det.is_pornographic,
				det.content_rating.as_deref(),
			);

			if let Some(desc) = det.description {
				let trimmed = desc.trim();
				if !trimmed.is_empty() {
					manga.description = Some(String::from(trimmed));
				}
			}

			// Consume series_staff once, partitioning into authors/artists so the
			// names don't need to be cloned. A staff member credited for both
			// story and art lands in both lists.
			let mut authors: Vec<String> = Vec::new();
			let mut artists: Vec<String> = Vec::new();
			for staff in det.series_staff {
				let role = staff.role.to_lowercase();
				let is_author = role.contains("author") || role.contains("story");
				let is_artist = role.contains("artist") || role.contains("art");
				match (is_author, is_artist) {
					(true, true) => {
						authors.push(staff.name.clone());
						artists.push(staff.name);
					}
					(true, false) => authors.push(staff.name),
					(false, true) => artists.push(staff.name),
					(false, false) => {}
				}
			}
			if !authors.is_empty() {
				manga.authors = Some(authors);
			}
			if !artists.is_empty() {
				manga.artists = Some(artists);
			}

			// The site lists genres and (separate) tags; surface both together,
			// skipping any flagged as spoilers.
			let tags: Vec<String> = det
				.genres
				.into_iter()
				.filter(|g| !g.is_spoiler)
				.map(|g| g.genre_name)
				.chain(
					det.tags
						.into_iter()
						.filter(|t| !t.is_spoiler)
						.map(|t| t.tag_name),
				)
				.collect();
			if !tags.is_empty() {
				manga.tags = Some(tags);
			}
		}

		if let Some(index) = index {
			let scanlator = find_source(&sources, det.source_id.as_deref()).map(scanlator);
			manga.chapters = Some(index.into_chapters(&manga.key, scanlator.as_deref()));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// Step 1: obtain a short-lived integrity token
		let integrity: IntegrityResponse = Request::post(format!("{}/api/integrity", base_url()))?
			.header("Content-Type", "application/json")
			.header("Origin", &base_url())
			.header("Referer", &format!("{}/", base_url()))
			.body(String::new())
			.json_owned()?;

		// Step 2: exchange the integrity token for an access token + page manifest
		let challenge: ChallengeResponse = Request::post(format!(
			"{}/books/{}?is_datasaver=false",
			api_base(),
			chapter.key
		))?
		.header("Content-Type", "application/json")
		.header("Origin", &base_url())
		.header("Referer", &format!("{}/", base_url()))
		.header("x-integrity-token", &integrity.token)
		.body(String::from("{}"))
		.json_owned()?;

		let cache_url = challenge.cache_url;
		let token = challenge.access_token;

		let mut pages: Vec<(i32, Page)> = match challenge.manifest {
			Some(manifest) => manifest
				.pages
				.into_iter()
				.map(|p| {
					let ext = p.ext.unwrap_or_else(|| String::from("jxl"));
					let url = format!(
						"{cache_url}/api/v2/books/page/{}/{}.{ext}?token={token}",
						chapter.key, p.page_id
					);
					(
						p.page_no,
						Page {
							content: PageContent::url(url),
							..Default::default()
						},
					)
				})
				.collect(),
			None => Vec::new(),
		};

		pages.sort_by_key(|(n, _)| *n);
		Ok(pages.into_iter().map(|(_, p)| p).collect())
	}
}

impl ListingProvider for Kagane {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"Recently Added" => "created_at",
			"Recently Updated" => "updated_at",
			_ => bail!("Unknown listing"),
		};
		search_page(
			&search_url(page, 35, sort, false),
			build_search_body(None, SearchFilters::default())?,
		)
	}
}

#[derive(Clone, Copy)]
enum Layout {
	Featured,
	Row,
	Chapters,
}

/// Home sections: title, sort and layout. The rows link to listings of the same name.
const SECTIONS: [(&str, &str, Layout); 3] = [
	("Popular This Week", "avg_views_week", Layout::Featured),
	("Recently Added", "created_at", Layout::Row),
	("Recently Updated", "updated_at", Layout::Chapters),
];

impl Home for Kagane {
	fn get_home(&self) -> Result<HomeLayout> {
		// The layout goes out first so the page shows while the sections load.
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: SECTIONS
				.iter()
				.map(|(title, _, layout)| HomeComponent {
					title: Some(String::from(*title)),
					subtitle: None,
					value: match layout {
						Layout::Featured => HomeComponentValue::empty_big_scroller(),
						Layout::Row => HomeComponentValue::empty_scroller(),
						Layout::Chapters => HomeComponentValue::empty_manga_chapter_list(),
					},
				})
				.collect(),
		}));

		let body = build_search_body(None, SearchFilters::default())?;
		let mut requests = SECTIONS
			.iter()
			.map(|(_, sort, _)| api_post(&search_url(1, 20, sort, false), body.clone()))
			.collect::<Result<Vec<_>>>()?;
		let show_source = show_source();
		if show_source {
			requests.push(sources_request()?);
		}
		let mut responses = Request::send_all(requests);
		let sources = if show_source {
			parse_sources(responses.pop())
		} else {
			Vec::new()
		};

		for ((title, _, layout), response) in SECTIONS.into_iter().zip(responses) {
			let Ok(page) = response
				.map_err(Into::into)
				.and_then(|response| response.get_json_owned::<SearchResponse>())
			else {
				continue;
			};
			let items = page.content.into_iter();
			let listing = Some(Listing {
				id: String::from(title),
				name: String::from(title),
				kind: ListingKind::Default,
			});
			let value = match layout {
				Layout::Featured => HomeComponentValue::BigScroller {
					entries: items.map(|item| item.into_manga(&sources)).collect(),
					auto_scroll_interval: None,
				},
				Layout::Row => HomeComponentValue::Scroller {
					entries: items.map(|item| item.into_manga(&sources).into()).collect(),
					listing,
				},
				Layout::Chapters => HomeComponentValue::MangaChapterList {
					entries: items
						.filter_map(|item| item.into_manga_with_chapter(&sources))
						.collect(),
					page_size: Some(5),
					listing,
				},
			};
			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(String::from(title)),
				subtitle: None,
				value,
			}));
		}

		Ok(HomeLayout::default())
	}
}

impl DynamicFilters for Kagane {
	// Sources come from the site, which adds them as it goes.
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut list: SourceList = sources_request()?.json_owned()?;
		list.sources.sort_by(|a, b| a.title.cmp(&b.title));
		let (ids, options) = list
			.sources
			.into_iter()
			.map(|source| (source.source_id.into(), source.title.into()))
			.unzip();
		Ok(vec![
			MultiSelectFilter {
				id: "sources".into(),
				title: Some("Sources".into()),
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		])
	}
}

impl DeepLinkHandler for Kagane {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// A link can come from any of the mirrors.
		if let Some((_, rest)) = url.split_once("/series/") {
			let slug = rest.split('/').next().unwrap_or(rest);
			let slug = slug.split('?').next().unwrap_or(slug);
			let slug = slug.split('#').next().unwrap_or(slug);
			if !slug.is_empty() {
				return Ok(Some(DeepLinkResult::Manga {
					key: String::from(slug),
				}));
			}
		}
		Ok(None)
	}
}

impl ImageRequestProvider for Kagane {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Origin", &base_url())
			.header("Referer", &format!("{}/", base_url())))
	}
}

register_source!(
	Kagane,
	ListingProvider,
	Home,
	DynamicFilters,
	DeepLinkHandler,
	ImageRequestProvider
);
