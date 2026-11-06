#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicListings, DynamicSettings, GroupSetting,
	HashMap, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult,
	NotificationHandler, Page, Result, Setting, Source, WebLoginHandler,
	alloc::{String, Vec, vec},
	imports::net::Request,
	prelude::*,
};
use html::{ChapterPage as _, MangaPage as _};
use net::{Url, aid_from_href, base_url, sid_from_href};

mod auth;
mod home;
mod html;
mod net;
mod series;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

pub fn create_request(path: &str) -> Result<Request> {
	let base_url = base_url()?;
	Ok(net::request_with_cookie(
		&format!("{base_url}{path}"),
		auth::cookie_header().as_deref(),
	)?
	.header("Origin", &base_url))
}

struct Wnacg;

impl Source for Wnacg {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<aidoku::FilterValue>,
	) -> Result<MangaPageResult> {
		let url = Url::from_query_or_filters(query.as_deref(), page, filters)?;
		let html = url.request()?.html()?;
		html.manga_page_result()
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let html = Url::manga(manga.key.clone()).request()?.html()?;

		if needs_details {
			html.manga_details(&mut manga)?;
		}

		if needs_chapters && html::is_series(&html) {
			// A series page shows no gallery; its chapter list comes from the
			// JSON endpoint the site's own reader uses.
			let json = Url::series_chapters(manga.key.clone())
				.request()?
				.string()?;
			manga.chapters = Some(series::parse_chapters(&json, &manga.key)?);
		} else if needs_chapters {
			// An ordinary gallery is one whole book with no chapter
			// structure of its own, so it always reports a single chapter.
			manga.chapters = Some(html.chapters(&manga.key)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// Each series chapter is an ordinary gallery, so chapter.key is its
		// aid; a plain gallery's single chapter key equals manga.key.
		let text = Url::chapter(chapter.key).request()?.string()?;
		let urls = text
			.split("\\\"")
			.filter(|s| s.starts_with("//"))
			.map(|s| format!("https:{}", s))
			.collect::<Vec<String>>();

		let mut pages: Vec<Page> = Vec::new();
		for url in urls.into_iter() {
			pages.push(Page {
				content: aidoku::PageContent::url(url),
				..Default::default()
			});
		}

		Ok(pages)
	}
}

impl ImageRequestProvider for Wnacg {
	fn get_image_request(
		&self,
		url: String,
		_context: Option<aidoku::PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header("Referer", &base_url()?))
	}
}

impl DeepLinkHandler for Wnacg {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(aid) = aid_from_href(&url) else {
			return Ok(None);
		};

		if url.contains("/photos-index-aid-") {
			return Ok(Some(DeepLinkResult::Manga { key: aid }));
		}

		// Series chapter: `/photos-slide-aid-<chapter>-sid-<series>.html`.
		if let Some(sid) = sid_from_href(&url) {
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: sid,
				key: aid,
			}));
		}

		// Every other reader entry maps to the gallery's single chapter.
		if url.contains("/photos-slide-aid-")
			|| url.contains("/photos-gallery-aid-")
			|| url.contains("/photos-slist-aid-")
		{
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: aid.clone(),
				key: aid,
			}));
		}

		Ok(None)
	}
}

impl ListingProvider for Wnacg {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let path = match listing.id.as_str() {
			"dayup" => format!("/albums-favorite_ranking-page-{}-type-day", page),
			"weekup" => format!("/albums-favorite_ranking-page-{}-type-week", page),
			"monthup" => format!("/albums-favorite_ranking-page-{}-type-month", page),
			"update" => format!("/albums-index-page-{}.html", page),
			"doujinshi" => format!("/albums-index-page-{}-cate-5.html", page),
			"single-volume" => format!("/albums-index-page-{}-cate-6.html", page),
			"magazine" => format!("/albums-index-page-{}-cate-7.html", page),
			"korean" => format!("/albums-index-page-{}-cate-19.html", page),
			"korean-series" => format!("/albums-index-page-{}-cate-38.html", page),
			_ => bail!("Invalid listing"),
		};

		let html = create_request(&path)?.html()?;
		html.manga_page_result()
	}
}

impl DynamicListings for Wnacg {
	/// The korean category returns 404 when logged out, so the two korean
	/// listings are only shown after login (the app refetches listings per
	/// the refreshes declared in settings.json).
	fn get_dynamic_listings(&self) -> Result<Vec<Listing>> {
		if !auth::is_logged_in() {
			return Ok(Vec::new());
		}
		Ok(vec![
			Listing {
				id: String::from("korean"),
				name: String::from("韩漫"),
				..Default::default()
			},
			Listing {
				id: String::from("korean-series"),
				name: String::from("韩漫合集"),
				..Default::default()
			},
		])
	}
}

impl WebLoginHandler for Wnacg {
	fn handle_web_login(&self, key: String, cookies: HashMap<String, String>) -> Result<bool> {
		if key != "login" {
			bail!("Invalid login key: {key}");
		}
		Ok(auth::capture_web_login(&cookies))
	}
}

impl NotificationHandler for Wnacg {
	fn handle_notification(&self, notification: String) {
		// Fires for both login and logout; `auth` tells them apart.
		if notification == "login" {
			auth::handle_login_notification();
		}
	}
}

impl DynamicSettings for Wnacg {
	/// Only reads defaults, never sends requests. Never return an empty list:
	/// that would disable every button on the settings page.
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let footer = if auth::is_logged_in() {
			match auth::account() {
				Some(name) => format!("已登录：{name}"),
				None => String::from("已登录"),
			}
		} else {
			String::from("尚未登录，登录后可以看韩漫。")
		};

		Ok(vec![
			GroupSetting {
				key: "accountInfo".into(),
				title: "账号信息".into(),
				items: Vec::new(),
				footer: Some(footer.into()),
				..Default::default()
			}
			.into(),
		])
	}
}

register_source!(
	Wnacg,
	Home,
	ListingProvider,
	DynamicListings,
	ImageRequestProvider,
	DeepLinkHandler,
	WebLoginHandler,
	NotificationHandler,
	DynamicSettings
);
