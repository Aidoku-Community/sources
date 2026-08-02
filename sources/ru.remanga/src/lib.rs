#![no_std]
extern crate alloc;

mod auth;
mod helpers;
mod models;
mod settings;

use crate::auth::{
	apply_token_from_settings, auth_hint, handle_web_login, is_logged_in, logout,
	refresh_account_info, stored_balance, stored_username, take_just_logged_in,
};
use crate::helpers::{fetch_chapters, fetch_manga_with_branches, fetch_pages, search};
use crate::settings::{SITE_URL, USER_AGENT};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::imports::std::send_partial_result;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicSettings, FilterValue, GroupSetting, HashMap,
	ImageRequestProvider, Manga, MangaPageResult, NotificationHandler, Page, PageContext, Result,
	Setting, Source, WebLoginHandler,
	alloc::{String, Vec, format, vec},
	prelude::*,
};

struct Remanga;

impl Source for Remanga {
	fn new() -> Self {
		// Enough headroom for paginated chapter lists without hammering the API.
		set_rate_limit(8, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		search(query, page, filters)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let (mut item, branches) = if needs_details {
			fetch_manga_with_branches(manga)
		} else {
			(manga, None)
		};

		if needs_chapters {
			if needs_details {
				send_partial_result(&item);
			}
			let key = item.key.clone();
			let chapters = fetch_chapters(&key, branches, |partial| {
				item.chapters = Some(partial.to_vec());
				send_partial_result(&item);
			})?;
			item.chapters = Some(chapters);
		}

		Ok(item)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		fetch_pages(&chapter.key)
	}
}

impl DeepLinkHandler for Remanga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Aidoku only forwards URLs matching `source.json` base URL.
		let Some(path) = url
			.strip_prefix(SITE_URL)
			.or_else(|| url.strip_prefix("https://remanga.org"))
			.map(|p| p.trim_start_matches('/'))
		else {
			return Ok(None);
		};

		let mut parts = path.split('/').filter(|p| !p.is_empty());
		let section = parts.next().unwrap_or("");
		if section != "manga" && section != "titles" && section != "title" {
			return Ok(None);
		}
		let Some(slug) = parts.next() else {
			return Ok(None);
		};
		if let Some(chapter) = parts.next()
			&& chapter.chars().all(|c| c.is_ascii_digit())
		{
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: slug.into(),
				key: chapter.into(),
			}));
		}
		Ok(Some(DeepLinkResult::Manga { key: slug.into() }))
	}
}

impl ImageRequestProvider for Remanga {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Referer", &format!("{SITE_URL}/"))
			.header("User-Agent", USER_AGENT))
	}
}

impl WebLoginHandler for Remanga {
	fn handle_web_login(&self, _key: String, cookies: HashMap<String, String>) -> Result<bool> {
		handle_web_login(cookies)
	}
}

impl NotificationHandler for Remanga {
	fn handle_notification(&self, notification: String) {
		match notification.as_str() {
			"login" => {
				if take_just_logged_in() || is_logged_in() {
					let _ = refresh_account_info();
				} else {
					logout();
				}
			}
			"token.changed"
				if !apply_token_from_settings().ok().unwrap_or(false) && !is_logged_in() =>
			{
				logout();
			}
			_ => {}
		}
	}
}

impl DynamicSettings for Remanga {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let footer = if is_logged_in() {
			let name = stored_username().unwrap_or_else(|| "аккаунт".into());
			match stored_balance() {
				Some(balance) => format!("Вход выполнен: {name}\nБаланс: {balance}"),
				None => format!("Вход выполнен: {name}"),
			}
		} else {
			auth_hint().unwrap_or_else(|| "Вход не выполнен.".into())
		};

		Ok(vec![
			GroupSetting {
				key: "accountStatus".into(),
				title: "Статус".into(),
				items: Vec::new(),
				footer: Some(footer.into()),
				..Default::default()
			}
			.into(),
		])
	}
}

register_source!(
	Remanga,
	DeepLinkHandler,
	ImageRequestProvider,
	WebLoginHandler,
	NotificationHandler,
	DynamicSettings
);
