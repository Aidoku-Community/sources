#![no_std]
extern crate alloc;

mod auth;
mod helpers;
mod keys;
mod models;
mod ranobe;
mod settings;

use crate::auth::{
	handle_web_login, is_logged_in, logout, refresh_username, stored_username, take_just_logged_in,
};
use crate::helpers::{
	apply_headers, fetch_by_id, fetch_chapter_pages, fetch_chapters, get_base_url, search,
};
use crate::keys::{Section, parse_key, ranobe_slug};
use crate::ranobe::{fetch_ranobe, fetch_ranobe_chapter_text, search_ranobe};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::imports::std::send_partial_result;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicSettings, FilterValue, GroupSetting, HashMap,
	ImageRequestProvider, Manga, MangaPageResult, MigrationHandler, NotificationHandler, Page,
	PageContent, PageContext, Result, Setting, Source, WebLoginHandler,
	alloc::{String, Vec, format, vec},
	prelude::*,
};

struct Desu;

impl Source for Desu {
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
		let mut section = Section::Manga;
		let mut rest = Vec::new();
		for filter in filters {
			match filter {
				FilterValue::Select { id, value } if id == "section" => {
					section = if value == "ranobe" {
						Section::Ranobe
					} else {
						Section::Manga
					};
				}
				other => rest.push(other),
			}
		}

		match section {
			Section::Ranobe => search_ranobe(query, page, rest),
			Section::Manga => {
				let result = search(query, page, rest)?;
				Ok(MangaPageResult {
					entries: result.entries,
					has_next_page: result.has_next_page,
				})
			}
		}
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let (section, id) = parse_key(&manga.key)?;
		match section {
			Section::Manga => {
				let mut item = if needs_details {
					fetch_by_id(id.as_str())?.into_manga(Some(manga), false, true)
				} else {
					manga
				};

				if needs_chapters {
					if needs_details {
						send_partial_result(&item);
					}
					item.chapters = Some(
						fetch_chapters(id.as_str())?
							.into_iter()
							.map(Chapter::from)
							.collect(),
					);
				}
				Ok(item)
			}
			Section::Ranobe => {
				let mut item = fetch_ranobe(id.as_str(), needs_details, needs_chapters)?;
				if !needs_details {
					item.title = manga.title;
					if item.cover.is_none() {
						item.cover = manga.cover;
					}
				}
				if needs_details && needs_chapters {
					let chapters = item.chapters.take();
					send_partial_result(&item);
					item.chapters = chapters;
				}
				Ok(item)
			}
		}
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let (section, id) = parse_key(&manga.key)?;
		match section {
			Section::Manga => Ok(fetch_chapter_pages(id.as_str(), chapter.key.as_str())?
				.into_iter()
				.map(|url| Page {
					content: PageContent::url(url),
					..Page::default()
				})
				.collect()),
			Section::Ranobe => {
				let fallback;
				let url = match chapter.url.as_deref().filter(|u| !u.is_empty()) {
					Some(url) => url,
					None => {
						fallback = format!(
							"{}/ranobe/{}/{}",
							get_base_url(),
							id,
							chapter.key.trim_start_matches('/')
						);
						fallback.as_str()
					}
				};
				let text = fetch_ranobe_chapter_text(url)?;
				Ok(vec![Page {
					content: PageContent::text(text),
					..Page::default()
				}])
			}
		}
	}
}

impl DeepLinkHandler for Desu {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		use crate::settings::path_on_site;

		let Some(path) = path_on_site(&url) else {
			return Ok(None);
		};
		let path = path.trim_start_matches('/');

		if path.starts_with("ranobe/") {
			let slug = ranobe_slug(path).ok_or(error!("Invalid ranobe URL"))?;
			return Ok(Some(DeepLinkResult::Manga {
				key: format!("r:{slug}"),
			}));
		}

		let manga_id = path
			.split('/')
			.skip_while(|&s| s == "manga" || s == "api")
			.find(|s| s.contains('.'))
			.and_then(|s| s.rsplit_once('.').map(|(_, id)| id))
			.or_else(|| {
				path.split('/')
					.find(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
			})
			.ok_or(error!("Invalid URL"))?;

		Ok(Some(DeepLinkResult::Manga {
			key: format!("m:{manga_id}"),
		}))
	}
}

impl ImageRequestProvider for Desu {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(apply_headers(Request::get(url)?))
	}
}

impl MigrationHandler for Desu {
	fn handle_manga_migration(&self, key: String) -> Result<String> {
		// v5: numeric manga ids became `m:{id}`; ranobe keys are already `r:{slug}`.
		if key.starts_with("m:") || key.starts_with("r:") {
			Ok(key)
		} else {
			Ok(format!("m:{key}"))
		}
	}

	fn handle_chapter_migration(&self, _manga_key: String, chapter_key: String) -> Result<String> {
		Ok(chapter_key)
	}
}

impl WebLoginHandler for Desu {
	fn handle_web_login(&self, _key: String, cookies: HashMap<String, String>) -> Result<bool> {
		handle_web_login(cookies)
	}
}

impl NotificationHandler for Desu {
	fn handle_notification(&self, notification: String) {
		if notification == "login" {
			if take_just_logged_in() {
				let _ = refresh_username();
			} else {
				logout();
			}
		}
	}
}

impl DynamicSettings for Desu {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		// Fill username lazily if the session exists but the name was not cached yet.
		if is_logged_in() && stored_username().is_none() {
			let _ = refresh_username();
		}

		let footer = if is_logged_in() {
			match stored_username() {
				Some(name) => format!("Вход выполнен: {name}"),
				None => "Вход выполнен.".into(),
			}
		} else {
			"Вход не выполнен.".into()
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
	Desu,
	DeepLinkHandler,
	ImageRequestProvider,
	WebLoginHandler,
	NotificationHandler,
	DynamicSettings,
	MigrationHandler
);
