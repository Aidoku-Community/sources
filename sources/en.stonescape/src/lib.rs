#![no_std]

mod models;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, HomePartialResult, ImageRequestProvider, Link, Listing, ListingKind,
	ListingProvider, Manga, MangaPageResult, MangaWithChapter, Page, PageContent, Result, Source,
	alloc::{String, Vec, format, vec},
	imports::{error::AidokuError, net::Request, std::send_partial_result},
	prelude::*,
};
use models::*;

pub const BASE_URL: &str = "https://stonescape.xyz";
pub const API_URL: &str = "https://stonescape.xyz/api";

struct StoneScape;

impl Source for StoneScape {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut url = format!("{API_URL}/series?page={page}&limit=24&contentType=manhwa");

		if let Some(q) = query.filter(|q| !q.is_empty()) {
			url.push_str("&search=");
			url.push_str(&q);
		}

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } => {
					if id == "status" && !value.is_empty() {
						url.push_str("&status=");
						url.push_str(&value);
					}
				}
				FilterValue::MultiSelect { id, included, .. } => {
					if id == "genres" && !included.is_empty() {
						url.push_str("&genres=");
						url.push_str(&included.join(","));
					}
				}
				_ => {}
			}
		}

		let bytes = Request::get(&url)?.data()?;
		let res: SeriesResponse =
			serde_json::from_slice(&bytes).map_err(|_| AidokuError::Unimplemented)?;

		let has_next_page = if let Some(pag) = res.pagination {
			pag.page.unwrap_or(1) < pag.total_pages.unwrap_or(1)
		} else {
			false
		};

		let entries = res.data.into_iter().map(Series::into_manga).collect();

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
		let slug = String::from(manga.key.strip_prefix("/series/").unwrap_or(&manga.key));

		if needs_details {
			let url = format!("{API_URL}/series/by-slug/{slug}");
			let bytes = Request::get(&url)?.data()?;
			let res: Series =
				serde_json::from_slice(&bytes).map_err(|_| AidokuError::Unimplemented)?;
			res.apply_details(&mut manga);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let url = format!("{API_URL}/series/by-slug/{slug}/chapters");
			let bytes = Request::get(&url)?.data()?;
			let res: ChapterListResponse =
				serde_json::from_slice(&bytes).map_err(|_| AidokuError::Unimplemented)?;

			let mut chapters: Vec<Chapter> = res
				.chapters
				.into_iter()
				.map(|c| c.into_chapter(&slug))
				.collect();

			chapters.reverse();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_id = chapter
			.key
			.strip_prefix("/chapters/")
			.unwrap_or(&chapter.key);
		let url = format!("{API_URL}/chapters/{chapter_id}/pages");
		let bytes = Request::get(&url)?.data()?;
		let res: ChapterDetails =
			serde_json::from_slice(&bytes).map_err(|_| AidokuError::Unimplemented)?;

		let page_list = res.pages.or(res.images).unwrap_or_default();

		let pages = page_list
			.into_iter()
			.map(|p| Page {
				content: PageContent::url(format!("{BASE_URL}{}", p.url)),
				..Default::default()
			})
			.collect();

		Ok(pages)
	}
}

impl ListingProvider for StoneScape {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"popular" => format!(
				"{API_URL}/series/popular?page={page}&period=week&contentType=manhwa&limit=24"
			),
			"latest" => format!("{API_URL}/series?page={page}&limit=24&contentType=manhwa"),
			_ => return Err(AidokuError::Unimplemented),
		};

		let bytes = Request::get(&url)?.data()?;
		let res: SeriesResponse =
			serde_json::from_slice(&bytes).map_err(|_| AidokuError::Unimplemented)?;

		let has_next_page = if let Some(pag) = res.pagination {
			pag.page.unwrap_or(1) < pag.total_pages.unwrap_or(1)
		} else {
			false
		};

		let entries = res.data.into_iter().map(Series::into_manga).collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

impl Home for StoneScape {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Popular".into()),
					subtitle: None,
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Latest Updates".into()),
					subtitle: None,
					value: HomeComponentValue::empty_manga_chapter_list(),
				},
			],
		}));

		let popular_url =
			format!("{API_URL}/series/popular?page=1&period=week&contentType=manhwa&limit=15");
		let popular_res: Result<SeriesResponse> = Request::get(&popular_url)
			.and_then(|r| r.data())
			.map_err(AidokuError::from)
			.and_then(|b| serde_json::from_slice(&b).map_err(|_| AidokuError::Unimplemented));
		if let Ok(popular_res) = popular_res {
			let entries: Vec<Link> = popular_res
				.data
				.into_iter()
				.map(|s| s.into_manga().into())
				.collect();

			if !entries.is_empty() {
				send_partial_result(&HomePartialResult::Component(HomeComponent {
					title: Some("Popular".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries,
						listing: Some(Listing {
							id: "popular".into(),
							name: "Popular".into(),
							kind: ListingKind::Default,
						}),
					},
				}));
			}
		}

		let latest_url = format!("{API_URL}/series?page=1&limit=20&contentType=manhwa");
		let latest_res: Result<SeriesResponse> = Request::get(&latest_url)
			.and_then(|r| r.data())
			.map_err(AidokuError::from)
			.and_then(|b| serde_json::from_slice(&b).map_err(|_| AidokuError::Unimplemented));
		if let Ok(latest_res) = latest_res {
			let entries: Vec<MangaWithChapter> = latest_res
				.data
				.into_iter()
				.filter_map(Series::into_manga_with_chapter)
				.collect();

			if !entries.is_empty() {
				send_partial_result(&HomePartialResult::Component(HomeComponent {
					title: Some("Latest Updates".into()),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						page_size: None,
						entries,
						listing: Some(Listing {
							id: "latest".into(),
							name: "Latest Updates".into(),
							kind: ListingKind::Default,
						}),
					},
				}));
			}
		}

		Ok(HomeLayout::default())
	}
}

impl ImageRequestProvider for StoneScape {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Referer", "https://stonescape.xyz/")
			.header("Origin", "https://stonescape.xyz"))
	}
}

impl DeepLinkHandler for StoneScape {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.starts_with(BASE_URL) {
			return Ok(None);
		}

		let key = &url[BASE_URL.len()..];

		if key.starts_with("/series/") {
			Ok(Some(DeepLinkResult::Manga { key: key.into() }))
		} else {
			Ok(None)
		}
	}
}

register_source!(
	StoneScape,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
