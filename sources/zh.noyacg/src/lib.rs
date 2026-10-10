#![no_std]
use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter,
	FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult,
	NotificationHandler, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};
use helpers::*;
use models::*;

const WEB_URL: &str = "https://noymanga.com";
// the api and image servers reject requests without the official app's user agent
const USER_AGENT: &str = "NoyAcg/3.0";

mod auth;
mod filters;
mod helpers;
mod home;
mod models;

struct NoyAcg;

impl Source for NoyAcg {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query {
			let adult = get_adult_mode();
			if page == 1
				&& let Some(result) = try_id_lookup(&query, adult)?
			{
				return Ok(result);
			}
			// keyword search ignores sort and finished, like the site's default mode
			return do_search(&query, "default", None, None, adult, page);
		}

		let mut sort = None;
		let mut finished = None;
		let mut leaderboard = None;
		let mut tag = None;
		let mut author = None;
		let mut adult = None;
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } if id == "author" && !value.is_empty() => {
					author = Some(value);
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"sort" => sort = Some(value),
					"leaderboard" if !value.is_empty() => leaderboard = Some(value),
					"genre" => tag = Some(value),
					_ => {}
				},
				FilterValue::MultiSelect { id, included, .. } => match id.as_str() {
					"tag" => tag = Some(included.join(" ")),
					"finished" if included.len() == 1 => finished = included.into_iter().next(),
					"rating" => adult = Some(adult_mode(&included)),
					_ => {}
				},
				_ => {}
			}
		}
		let adult = adult.unwrap_or_else(get_adult_mode);
		let sort = sort.as_deref().unwrap_or("new");
		let finished = finished.as_deref();

		if let Some(author) = author {
			return do_search(&author, "author", Some(sort), finished, adult, page);
		}
		if let Some(tag) = tag {
			return do_search(&tag, "tag", Some(sort), finished, adult, page);
		}
		if let Some(id) = leaderboard {
			return get_listing(&id, adult, page);
		}
		let body = match finished {
			Some(finished) => format!("page={page}&sort={sort}&finished={finished}"),
			None => format!("page={page}&sort={sort}"),
		};
		fetch_listing("/api/b1/booklist", &body, adult, page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut resp = fetch_detail(&manga.key, get_adult_mode())?;

		if needs_chapters {
			manga.chapters = Some(resp.take_chapters(&manga.key));
		}
		if needs_details {
			let details = resp
				.into_manga(&manga.key)
				.ok_or_else(|| error!("無法取得漫畫資料"))?;
			manga.copy_from(details);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let (manga_id, chapter_id) = match chapter.key.split_once('/') {
			Some((mid, cid)) => (mid, Some(cid)),
			None => (chapter.key.as_str(), None),
		};

		let count = fetch_detail(manga_id, get_adult_mode())?
			.page_count(chapter_id)
			.filter(|&count| count > 0)
			.ok_or_else(|| error!("無法取得頁面資料"))?;

		let img_base = get_img_base();
		Ok((1..=count)
			.map(|i| Page {
				content: PageContent::url(format!("{img_base}/{}/{i}.webp", chapter.key)),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for NoyAcg {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		get_listing(&listing.id, get_adult_mode(), page)
	}
}

impl DeepLinkHandler for NoyAcg {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(key) = extract_manga_id(&url) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
		if let Some(path) = extract_reader_path(&url) {
			if let Some((manga_key, chapter_key)) = path.split_once('/') {
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: if chapter_key == "0" {
						manga_key.into()
					} else {
						path
					},
				}));
			}
			return Ok(Some(DeepLinkResult::Manga { key: path }));
		}
		Ok(None)
	}
}

impl ImageRequestProvider for NoyAcg {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("User-Agent", USER_AGENT))
	}
}

impl BasicLoginHandler for NoyAcg {
	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		auth::login(&username, &password)
	}
}

impl NotificationHandler for NoyAcg {
	fn handle_notification(&self, notification: String) {
		if notification == "login" && !auth::is_logged_in() {
			auth::logout();
		}
	}
}

impl DynamicFilters for NoyAcg {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		Ok(vec![filters::build_tag_filter(get_adult_mode())])
	}
}

fn get_listing(id: &str, adult: &str, page: i32) -> Result<MangaPageResult> {
	let (path, body) = match id {
		"latest" => ("/api/b1/booklist", format!("page={page}&sort=new")),
		"completed" => (
			"/api/b1/booklist",
			format!("page={page}&sort=new&finished=true"),
		),
		"proportion" => ("/api/proportion", format!("page={page}")),
		"favorite" => ("/api/v4/favorites/get", format!("page={page}")),
		"random" => ("/api/v4/book/random", String::new()),
		id => {
			let (path, period) = if let Some(period) = id.strip_prefix("read:") {
				("/api/readLeaderboard", period)
			} else if let Some(period) = id.strip_prefix("fav:") {
				("/api/favLeaderboard", period)
			} else {
				bail!("未知的列表類型");
			};
			(path, format!("type={period}&page={page}"))
		}
	};
	fetch_listing(path, &body, adult, page)
}

fn do_search(
	value: &str,
	mode: &str,
	sort: Option<&str>,
	finished: Option<&str>,
	adult: &str,
	page: i32,
) -> Result<MangaPageResult> {
	let mut body = QueryParameters::new();
	body.push("value", Some(value));
	body.push("page", Some(&page.to_string()));
	body.push("type", Some("book"));
	body.push("mode", Some(mode));
	if let Some(sort) = sort {
		let sort = match sort {
			"new" | "upload" => "time",
			other => other,
		};
		body.push("sort", Some(sort));
	}
	if finished.is_some() {
		body.push("finished", finished);
	}
	let body = body.to_string();
	let resp: SearchResp = fetch_json(|| api_post("/api/v4/search/fetch", &body, adult))?;
	Ok(resp.into_page_result(page))
}

// the official app opens `NID<id>` queries as a book; links to a book work the same way
fn try_id_lookup(query: &str, adult: &str) -> Result<Option<MangaPageResult>> {
	let nid = query
		.split_at_checked(3)
		.filter(|(prefix, id)| {
			prefix.eq_ignore_ascii_case("nid")
				&& !id.is_empty()
				&& id.bytes().all(|b| b.is_ascii_digit())
		})
		.map(|(_, id)| id.into());
	let Some(key) = nid.or_else(|| extract_manga_id(query)) else {
		return Ok(None);
	};
	// unknown ids return the deleted placeholder book
	let entries = fetch_detail(&key, adult)?
		.book
		.and_then(|b| b.info)
		.filter(|m| !m.is_deleted())
		.map(|m| vec![m.into_basic_manga(&get_img_base())])
		.unwrap_or_default();
	Ok(Some(MangaPageResult {
		entries,
		..Default::default()
	}))
}

fn fetch_listing(path: &str, body: &str, adult: &str, page: i32) -> Result<MangaPageResult> {
	let resp: ListingResp = fetch_json(|| api_post(path, body, adult))?;
	Ok(resp.into_page_result(page))
}

fn fetch_detail(id: &str, adult: &str) -> Result<BookDetailResp> {
	let path = format!("/api/v4/book/{id}?comment=false");
	fetch_json(|| api_get(&path, adult))
}

register_source!(
	NoyAcg,
	Home,
	ListingProvider,
	DeepLinkHandler,
	ImageRequestProvider,
	BasicLoginHandler,
	NotificationHandler,
	DynamicFilters
);
