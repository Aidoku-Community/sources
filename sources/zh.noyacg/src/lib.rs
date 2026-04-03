#![no_std]
use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter,
	FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult,
	NotificationHandler, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::net::Request,
	prelude::*,
};

mod auth;
mod filters;
mod helpers;
mod home;
mod models;

use auth::ensure_session;
use helpers::*;
use models::*;

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
		ensure_session()?;
		let keyword = query.unwrap_or_default();
		let (sort, leaderboard, tag, finished, author_query, rating_override) =
			Self::parse_filters(&filters);
		let adult = rating_override.unwrap_or_else(get_adult_mode);

		if !author_query.is_empty() {
			return finish_search(
				self.do_search(author_query, "author", sort, &adult, page)?,
				page,
			);
		}

		if !tag.is_empty() {
			return finish_search(self.do_search(&tag, "tag", sort, &adult, page)?, page);
		}

		if !keyword.is_empty() {
			if page <= 1
				&& let Some(result) = self.try_id_lookup(&keyword, &adult)?
			{
				return Ok(result);
			}
			let tag_resp = self.do_search(&keyword, "tag", sort, &adult, page)?;
			if !tag_resp.entries.is_empty() {
				return Ok(tag_resp);
			}
			return finish_search(
				self.do_search(&keyword, "default", sort, &adult, page)?,
				page,
			);
		}

		let base_url = get_base_url();
		let referer = format!("{base_url}/");
		let page_str = page.to_string();

		if !leaderboard.is_empty() {
			let (endpoint, lb_type) = if let Some(t) = leaderboard.strip_prefix("read:") {
				("readLeaderboard", t)
			} else if let Some(t) = leaderboard.strip_prefix("fav:") {
				("favLeaderboard", t)
			} else {
				("readLeaderboard", leaderboard)
			};
			let body = build_form_body(&[("type", lb_type), ("page", &page_str)]);
			return post_form_listing(
				&format!("{base_url}/api/{endpoint}"),
				&body,
				&referer,
				&adult,
				page,
			);
		}

		let body = build_form_body(&[("page", &page_str), ("sort", sort), ("finished", &finished)]);
		let result = post_form_listing(
			&format!("{base_url}/api/b1/booklist"),
			&body,
			&referer,
			&adult,
			page,
		)?;
		if page <= 1 && result.entries.is_empty() && !auth::is_logged_in() {
			bail!("請先登入以檢視內容");
		}
		Ok(result)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		ensure_session()?;
		let base_url = get_base_url();
		let mut resp: BookDetailResp =
			Request::get(format!("{base_url}/api/v4/book/{}", manga.key))?
				.header("Referer", &format!("{base_url}/"))
				.header("allow-adult", &get_adult_mode())
				.json_owned()?;

		if needs_chapters {
			manga.chapters = Some(resp.take_chapters(&manga.key));
		}
		if needs_details {
			manga.copy_from(resp.into_manga(&manga.key));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		ensure_session()?;
		let (manga_id, chapter_id) = match chapter.key.split_once('/') {
			Some((mid, cid)) => (mid, Some(cid)),
			None => (chapter.key.as_str(), None),
		};

		let base_url = get_base_url();
		let detail: BookDetailResp = Request::get(format!("{base_url}/api/v4/book/{manga_id}"))?
			.header("Referer", &format!("{base_url}/"))
			.header("allow-adult", &get_adult_mode())
			.json_owned()?;

		let count = match chapter_id {
			Some(cid) => detail.find_chapter_page_count(cid).unwrap_or(0),
			None => detail.page_count(),
		};
		if count == 0 {
			bail!("無法取得頁面資料");
		}

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
		ensure_session()?;
		let adult = get_adult_mode();
		let base_url = get_base_url();
		let referer = format!("{base_url}/");
		let page_str = page.to_string();

		let (url, body) = match listing.id.as_str() {
			"latest" => (
				format!("{base_url}/api/b1/booklist"),
				build_form_body(&[("page", &page_str), ("sort", "new")]),
			),
			"completed" => (
				format!("{base_url}/api/b1/booklist"),
				build_form_body(&[("page", &page_str), ("sort", "new"), ("finished", "true")]),
			),
			"proportion" => (
				format!("{base_url}/api/proportion"),
				build_form_body(&[("page", &page_str)]),
			),
			"favorite" => {
				if !auth::is_logged_in() {
					bail!("請先登入以使用收藏功能");
				}
				let body = build_form_body(&[("page", &page_str)]);
				let resp: FavoritesResp = post_with_form(
					&format!("{base_url}/api/v4/favorites/get"),
					&body,
					&referer,
					&adult,
				)?
				.json_owned()?;
				let result = resp.into_page_result(page);
				if page <= 1 && result.entries.is_empty() {
					bail!("呢度乜都冇");
				}
				return Ok(result);
			}
			id if id.starts_with("leaderboard:") => {
				let lb_type = &id["leaderboard:".len()..];
				(
					format!("{base_url}/api/readLeaderboard"),
					build_form_body(&[("type", lb_type), ("page", &page_str)]),
				)
			}
			id if id.starts_with("fav_leaderboard:") => {
				let lb_type = &id["fav_leaderboard:".len()..];
				(
					format!("{base_url}/api/favLeaderboard"),
					build_form_body(&[("type", lb_type), ("page", &page_str)]),
				)
			}
			"random" => {
				let resp: ListingResp = Request::post(format!("{base_url}/api/v4/book/random"))?
					.header("Referer", &referer)
					.header("allow-adult", &adult)
					.json_owned()?;
				return Ok(resp.into_random_result());
			}
			_ => bail!("未知的列表類型"),
		};
		post_form_listing(&url, &body, &referer, &adult, page)
	}
}

impl DeepLinkHandler for NoyAcg {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(key) = extract_manga_id(&url) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
		if let Some(path) = extract_reader_path(&url) {
			if let Some((manga_key, _chapter_key)) = path.split_once('/') {
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: path,
				}));
			}
			return Ok(Some(DeepLinkResult::Manga { key: path }));
		}
		Ok(None)
	}
}

impl ImageRequestProvider for NoyAcg {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{}/", get_base_url())))
	}
}

impl BasicLoginHandler for NoyAcg {
	fn handle_basic_login(&self, key: String, username: String, password: String) -> Result<bool> {
		if key != "login" {
			bail!("登入入口無效");
		}
		if username.is_empty() || password.is_empty() {
			return Ok(false);
		}
		let ok = auth::do_login(&username, &password)?;
		if ok {
			auth::store_credentials(&username, &password);
			auth::set_just_logged_in();
		}
		Ok(ok)
	}
}

impl NotificationHandler for NoyAcg {
	fn handle_notification(&self, notification: String) {
		if notification.as_str() == "login" {
			if auth::is_just_logged_in() {
				auth::clear_just_logged_in();
			} else {
				auth::clear_credentials();
			}
		}
	}
}

impl DynamicFilters for NoyAcg {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let adult_mode = get_adult_mode();
		Ok(vec![filters::build_tag_filter(&adult_mode)])
	}
}

impl NoyAcg {
	fn parse_filters(
		filters: &[FilterValue],
	) -> (&str, &str, String, String, &str, Option<String>) {
		let mut sort = "new";
		let mut leaderboard = "";
		let mut tag = String::new();
		let mut finished = String::new();
		let mut author_query = "";
		let mut rating_override: Option<String> = None;
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } if id == "author" && !value.is_empty() => {
					author_query = value;
				}
				FilterValue::Select { id, value } if !value.is_empty() => match id.as_str() {
					"sort" => sort = value,
					"leaderboard" => leaderboard = value,
					"tag" | "genre" => tag = value.clone(),
					_ => {}
				},
				FilterValue::MultiSelect { id, included, .. }
					if (id == "tag" || id == "genre") && !included.is_empty() =>
				{
					tag = included.join(" ");
				}
				FilterValue::MultiSelect { id, included, .. } if id == "finished" => {
					if included.len() == 1 {
						finished = included[0].clone();
					}
				}
				FilterValue::MultiSelect { id, included, .. } if id == "rating" => {
					if !included.is_empty() {
						let has_sfw = included.iter().any(|s| s == "false");
						let has_nsfw = included.iter().any(|s| s == "true");
						rating_override = Some(match (has_sfw, has_nsfw) {
							(true, true) => "both".into(),
							(false, true) => "true".into(),
							_ => "false".into(),
						});
					}
				}
				_ => {}
			}
		}
		(
			sort,
			leaderboard,
			tag,
			finished,
			author_query,
			rating_override,
		)
	}

	fn do_search(
		&self,
		value: &str,
		mode: &str,
		sort: &str,
		adult: &str,
		page: i32,
	) -> Result<MangaPageResult> {
		let search_sort = match sort {
			"new" | "upload" => "time",
			"views" => "read",
			other => other,
		};

		let base_url = get_base_url();
		let body = build_form_body(&[
			("value", value),
			("page", &page.to_string()),
			("type", "book"),
			("mode", mode),
			("sort", search_sort),
		]);

		let resp: SearchResp = post_with_form(
			&format!("{base_url}/api/v4/search/fetch"),
			&body,
			&format!("{base_url}/"),
			adult,
		)?
		.json_owned()?;

		Ok(resp.into_page_result(page))
	}

	fn try_id_lookup(&self, query: &str, adult: &str) -> Result<Option<MangaPageResult>> {
		let trimmed = query.trim();
		let key: String = if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
			trimmed.into()
		} else if let Some(id) = extract_manga_id(trimmed) {
			id
		} else {
			return Ok(None);
		};

		let base_url = get_base_url();
		let resp: BookDetailResp = Request::get(format!("{base_url}/api/v4/book/{key}"))?
			.header("Referer", &format!("{base_url}/"))
			.header("allow-adult", adult)
			.json_owned()?;

		let is_deleted = resp
			.book
			.as_ref()
			.and_then(|b| b.info.as_ref())
			.is_some_and(|m| m.is_deleted());
		if is_deleted {
			bail!("呢度乜都冇");
		}

		let manga = resp.into_manga(&key);
		if manga.title.is_empty() {
			return Ok(None);
		}
		Ok(Some(MangaPageResult {
			has_next_page: false,
			entries: vec![manga],
		}))
	}
}

fn finish_search(result: MangaPageResult, page: i32) -> Result<MangaPageResult> {
	if page <= 1 && result.entries.is_empty() {
		bail!("呢度乜都冇");
	}
	Ok(result)
}

fn post_form_listing(
	url: &str,
	body: &str,
	referer: &str,
	adult: &str,
	page: i32,
) -> Result<MangaPageResult> {
	let resp: ListingResp = post_with_form(url, body, referer, adult)?.json_owned()?;
	Ok(resp.into_page_result(page))
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
