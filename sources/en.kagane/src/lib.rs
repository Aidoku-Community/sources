#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, ImageRequestProvider, Link, Listing, ListingKind, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{format, string::String, vec, vec::Vec},
	imports::{
		net::{Request, TimeUnit, set_rate_limit},
		std::parse_date,
	},
	prelude::*,
};

mod helpers;
mod models;
use helpers::*;
use models::*;

const BASE_URL: &str = "https://kagane.to";
const API_BASE: &str = "https://yuzuki.kagane.to/api/v2";

// Sort param by filter index (from filters.json options order).
const SORT_PARAMS: &[&str] = &[
	"total_views", // 0: Popular (Total Views)
	"updated_at",  // 1: Latest Updated
	"series_name", // 2: By Name
	"created_at",  // 3: Newest
];

fn api_get(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Origin", BASE_URL)
		.header("Referer", &format!("{BASE_URL}/")))
}

fn api_post(url: &str, body: String) -> Result<Request> {
	Ok(Request::post(url)?
		.header("Content-Type", "application/json")
		.header("Origin", BASE_URL)
		.header("Referer", &format!("{BASE_URL}/"))
		.body(body))
}

struct Kagane;

impl Source for Kagane {
	fn new() -> Self {
		set_rate_limit(2, 2, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut sort_idx = 0usize;
		let mut statuses: Vec<String> = Vec::new();
		let mut formats: Vec<String> = Vec::new();

		for filter in &filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					sort_idx = *index as usize;
				}
				FilterValue::MultiSelect { id, included, .. } if id == "status" => {
					statuses = included.clone();
				}
				FilterValue::MultiSelect { id, included, .. } if id == "format" => {
					formats = included.clone();
				}
				_ => {}
			}
		}

		let sort = SORT_PARAMS.get(sort_idx).unwrap_or(&"total_views");
		let url = format!(
			"{API_BASE}/search/series?page={}&size=35&sort={},desc",
			page - 1,
			sort
		);
		let body = build_search_body(query.as_deref(), &statuses, &formats);
		let resp: SearchResponse = api_post(&url, body)?.json_owned()?;

		let has_next_page = !resp.last;
		let entries = resp.content.into_iter().map(Manga::from).collect();
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
		let det: SeriesDetail =
			api_get(&format!("{API_BASE}/series/{}", manga.key))?.json_owned()?;

		if needs_details {
			manga.title = String::from(det.title.trim());
			manga.cover = det
				.series_covers
				.first()
				.map(|c| format!("{API_BASE}/image/{}", c.image_id));
			manga.url = Some(format!("{BASE_URL}/series/{}", manga.key));
			manga.status = parse_status(&det.upload_status);
			manga.viewer = parse_viewer(det.format.as_deref());
			manga.content_rating = parse_content_rating(det.content_rating.as_deref());

			if let Some(desc) = det.description {
				let trimmed = desc.trim();
				if !trimmed.is_empty() {
					manga.description = Some(String::from(trimmed));
				}
			}

			let authors: Vec<String> = det
				.series_staff
				.iter()
				.filter(|s| {
					let role = s.role.to_lowercase();
					role.contains("author") || role.contains("story")
				})
				.map(|s| s.name.clone())
				.collect();
			if !authors.is_empty() {
				manga.authors = Some(authors);
			}

			let artists: Vec<String> = det
				.series_staff
				.iter()
				.filter(|s| {
					let role = s.role.to_lowercase();
					role.contains("artist") || role.contains("art")
				})
				.map(|s| s.name.clone())
				.collect();
			if !artists.is_empty() {
				manga.artists = Some(artists);
			}

			let tags: Vec<String> = det.genres.iter().map(|g| g.genre_name.clone()).collect();
			if !tags.is_empty() {
				manga.tags = Some(tags);
			}
		}

		if needs_chapters {
			let mut chapters: Vec<Chapter> = det
				.series_books
				.into_iter()
				.map(|book| {
					let url = format!("{BASE_URL}/series/{}/reader/{}", manga.key, book.book_id);
					let scanlators: Vec<String> =
						book.groups.into_iter().map(|g| g.title).collect();
					Chapter {
						key: book.book_id,
						chapter_number: book.chapter_no.as_deref().and_then(|s| s.parse().ok()),
						volume_number: book.volume_no.as_deref().and_then(|s| s.parse().ok()),
						title: {
							let t = book.title.trim();
							if t.is_empty() {
								None
							} else {
								Some(String::from(t))
							}
						},
						date_uploaded: book.created_at.as_deref().and_then(|s| {
							let s = s.split_once('.').map_or(s, |(b, _)| b);
							parse_date(format!("{s}Z"), "yyyy-MM-dd'T'HH:mm:ss'Z'")
						}),
						scanlators: if scanlators.is_empty() {
							None
						} else {
							Some(scanlators)
						},
						url: Some(url),
						..Default::default()
					}
				})
				.collect();
			// API returns chapters oldest-first; reverse for newest-first display.
			chapters.reverse();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// Step 1: obtain a short-lived integrity token
		let integrity: IntegrityResponse = Request::post(format!("{BASE_URL}/api/integrity"))?
			.header("Content-Type", "application/json")
			.header("Origin", BASE_URL)
			.header("Referer", &format!("{BASE_URL}/"))
			.body(String::new())
			.json_owned()?;

		// Step 2: exchange the integrity token for an access token + page manifest
		let challenge: ChallengeResponse = Request::post(format!(
			"{API_BASE}/books/{}?is_datasaver=false",
			chapter.key
		))?
		.header("Content-Type", "application/json")
		.header("Origin", BASE_URL)
		.header("Referer", &format!("{BASE_URL}/"))
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
			"Popular Today" => "avg_views_day",
			"Popular This Week" => "avg_views_week",
			"Popular This Month" => "avg_views_month",
			"Popular All Time" => "total_views",
			"Latest" => "updated_at",
			"Recently Added" => "created_at",
			_ => "total_views",
		};
		let url = format!(
			"{API_BASE}/search/series?page={}&size=35&sort={},desc",
			page - 1,
			sort
		);
		let body = build_search_body(None, &[], &[]);
		let resp: SearchResponse = api_post(&url, body)?.json_owned()?;

		let has_next_page = !resp.last;
		let entries = resp.content.into_iter().map(Manga::from).collect();
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

impl Home for Kagane {
	fn get_home(&self) -> Result<HomeLayout> {
		let pop_url = format!("{API_BASE}/search/series?page=0&size=20&sort=avg_views_day,desc");
		let lat_url = format!("{API_BASE}/search/series?page=0&size=20&sort=updated_at,desc");
		let body = build_search_body(None, &[], &[]);

		let popular: Vec<Link> = api_post(&pop_url, body.clone())?
			.json_owned::<SearchResponse>()?
			.content
			.into_iter()
			.map(|s| Manga::from(s).into())
			.collect();

		let latest: Vec<Link> = api_post(&lat_url, body)?
			.json_owned::<SearchResponse>()?
			.content
			.into_iter()
			.map(|s| Manga::from(s).into())
			.collect();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some(String::from("Popular Today")),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: popular,
						listing: Some(Listing {
							id: String::from("Popular Today"),
							name: String::from("Popular Today"),
							kind: ListingKind::Default,
						}),
					},
				},
				HomeComponent {
					title: Some(String::from("Latest Updates")),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: latest,
						listing: Some(Listing {
							id: String::from("Latest"),
							name: String::from("Latest"),
							kind: ListingKind::Default,
						}),
					},
				},
			],
		})
	}
}

impl DeepLinkHandler for Kagane {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let prefix = format!("{BASE_URL}/series/");
		if let Some(rest) = url.strip_prefix(&prefix) {
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
			.header("Origin", BASE_URL)
			.header("Referer", &format!("{BASE_URL}/")))
	}
}

register_source!(
	Kagane,
	ListingProvider,
	Home,
	DeepLinkHandler,
	ImageRequestProvider
);
