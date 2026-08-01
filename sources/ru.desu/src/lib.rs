#![no_std]
extern crate alloc;

mod auth;
mod helpers;
mod keys;
mod models;
mod ranobe;
mod settings;

use crate::auth::{is_logged_in, login, logout, stored_username, take_just_logged_in};
use crate::helpers::{
	apply_headers, fetch_by_id, fetch_chapter_pages, fetch_chapters, get_base_url, search,
};
use crate::keys::{Section, parse_key, ranobe_slug};
use crate::ranobe::{fetch_ranobe, fetch_ranobe_chapter_text, listing_manga, search_ranobe};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::imports::std::send_partial_result;
use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, DynamicSettings, FilterValue,
	GroupSetting, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult,
	NotificationHandler, Page, PageContent, PageContext, Result, Setting, Source,
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
			Section::Ranobe => search_ranobe(query, page),
			Section::Manga => {
				let result = search(query, page, rest)?;
				Ok(MangaPageResult {
					has_next_page: result.has_next_page,
					entries: result
						.entries
						.into_iter()
						.map(|m| m.into_manga(None, true, false))
						.collect(),
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
				let url = chapter.url.clone().unwrap_or_else(|| {
					format!(
						"{}/ranobe/{}/{}",
						get_base_url(),
						id,
						chapter.key.trim_start_matches('/')
					)
				});
				let text = fetch_ranobe_chapter_text(&url)?;
				Ok(vec![Page {
					content: PageContent::text(text),
					..Page::default()
				}])
			}
		}
	}
}

impl ListingProvider for Desu {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"manga" => listing_manga(page),
			"ranobe" => search_ranobe(None, page),
			_ => bail!("Неизвестный раздел"),
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

impl BasicLoginHandler for Desu {
	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		login(&username, &password)
	}
}

impl NotificationHandler for Desu {
	fn handle_notification(&self, notification: String) {
		if notification == "login" {
			if take_just_logged_in() {
				// Login just succeeded; keep cookies.
			} else {
				logout();
			}
		}
	}
}

impl DynamicSettings for Desu {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let footer = if is_logged_in() {
			match stored_username() {
				Some(name) => format!(
					"Вход выполнен: {name}\nМанга доступна без входа, ранобэ — только с аккаунтом."
				),
				None => {
					"Вход выполнен.\nМанга доступна без входа, ранобэ — только с аккаунтом.".into()
				}
			}
		} else {
			"Вход не выполнен.\nРанобэ недоступно без авторизации на Desu.".into()
		};

		Ok(vec![
			GroupSetting {
				key: "accountStatus".into(),
				title: "Статус аккаунта".into(),
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
	ListingProvider,
	BasicLoginHandler,
	NotificationHandler,
	DynamicSettings
);
