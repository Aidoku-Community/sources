#![no_std]

mod models;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, HomePartialResult, ImageRequestProvider, Link, Listing, ListingKind,
	ListingProvider, Manga, MangaPageResult, MangaWithChapter, Page, PageContent, Result, Source,
	alloc::{String, Vec, format, vec},
	helpers::uri::encode_uri_component,
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

		if let Some(q) = query {
			url.push_str("&search=");
			url.push_str(&encode_uri_component(q));
		}

		for filter in filters {
			match filter {
				FilterValue::Select { id, value } if id == "status" && !value.is_empty() => {
					url.push_str("&status=");
					url.push_str(&value);
				}
				FilterValue::MultiSelect { id, included, .. }
					if id == "genres" && !included.is_empty() =>
				{
					url.push_str("&genres=");
					url.push_str(&included.join(","));
				}
				_ => {}
			}
		}

		let res: SeriesResponse = Request::get(&url)?.json_owned()?;

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
		if needs_details {
			let url = format!("{API_URL}/series/by-slug/{}", manga.key);
			let res: Series = Request::get(&url)?.json_owned()?;
			res.apply_details(&mut manga);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let url = format!("{API_URL}/series/by-slug/{}/chapters", manga.key);
			let res: ChapterListResponse = Request::get(&url)?.json_owned()?;

			let chapters: Vec<Chapter> = res
				.chapters
				.into_iter()
				.rev()
				.map(|c| c.into_chapter(&manga.key))
				.collect();

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{API_URL}/chapters/{}/pages", chapter.key);
		let res: ChapterDetails = Request::get(&url)?.json_owned()?;

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

		let res: SeriesResponse = Request::get(&url)?.json_owned()?;

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
					title: Some("Featured".into()),
					subtitle: None,
					value: HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("Popular Series".into()),
					subtitle: None,
					value: HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Latest Releases".into()),
					subtitle: None,
					value: HomeComponentValue::empty_manga_chapter_list(),
				},
			],
		}));

		let requests = Request::send_all([
			Request::get(format!("{API_URL}/banner-config"))?,
			Request::get(format!(
				"{API_URL}/series/popular?page=1&period=week&contentType=manhwa&limit=15"
			))?,
			Request::get(format!(
				"{API_URL}/series?page=1&limit=20&contentType=manhwa"
			))?,
		]);

		let mut req_iter = requests.into_iter();

		let banner_res: Option<BannerResponse> = req_iter
			.next()
			.and_then(|r| r.ok())
			.and_then(|r| r.get_json_owned().ok());

		let popular_res: Option<SeriesResponse> = req_iter
			.next()
			.and_then(|r| r.ok())
			.and_then(|r| r.get_json_owned().ok());

		let latest_res: Option<SeriesResponse> = req_iter
			.next()
			.and_then(|r| r.ok())
			.and_then(|r| r.get_json_owned().ok());

		if let Some(banner_res) = banner_res {
			let banner_entries: Vec<Manga> = banner_res
				.featured_series
				.into_iter()
				.map(Series::into_banner_manga)
				.collect();

			if !banner_entries.is_empty() {
				send_partial_result(&HomePartialResult::Component(HomeComponent {
					title: Some("Featured".into()),
					subtitle: None,
					value: HomeComponentValue::BigScroller {
						entries: banner_entries,
						auto_scroll_interval: Some(8.0),
					},
				}));
			}
		}

		if let Some(popular_res) = popular_res {
			let popular_entries: Vec<Link> = popular_res
				.data
				.into_iter()
				.map(|s| s.into_manga().into())
				.collect();

			if !popular_entries.is_empty() {
				send_partial_result(&HomePartialResult::Component(HomeComponent {
					title: Some("Popular Series".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: popular_entries,
						listing: Some(Listing {
							id: "popular".into(),
							name: "Popular Series".into(),
							kind: ListingKind::Default,
						}),
					},
				}));
			}
		}

		if let Some(latest_res) = latest_res {
			let latest_entries: Vec<MangaWithChapter> = latest_res
				.data
				.into_iter()
				.filter_map(Series::into_manga_with_chapter)
				.collect();

			if !latest_entries.is_empty() {
				send_partial_result(&HomePartialResult::Component(HomeComponent {
					title: Some("Latest Releases".into()),
					subtitle: None,
					value: HomeComponentValue::MangaChapterList {
						page_size: None,
						entries: latest_entries,
						listing: Some(Listing {
							id: "latest".into(),
							name: "Latest Releases".into(),
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
			.header("Origin", BASE_URL))
	}
}

impl DeepLinkHandler for StoneScape {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.strip_prefix(BASE_URL).unwrap_or(&url);

		if let Some(slug) = path.strip_prefix("/series/") {
			let slug = slug.split('/').next().unwrap_or(slug);
			Ok(Some(DeepLinkResult::Manga { key: slug.into() }))
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
