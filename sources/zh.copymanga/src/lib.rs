#![no_std]

mod auth;
mod favorites;
mod home;
mod html;
mod json;
mod net;
mod tests;

use aidoku::{
	BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter,
	FilterValue, Listing, ListingProvider, Manga, MangaPageResult, NotificationHandler, Page,
	Result, Source,
	alloc::{String, Vec},
	bail, error,
	imports::std::send_partial_result,
	println, register_source,
};
use html::{ChapterPage as _, FiltersPage as _, GenresPage as _, KeyPage as _, MangaPage as _};
use json::{chapter_list, search};
use net::Url;

pub(crate) struct Copymanga;

impl Source for Copymanga {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = Url::from_query_or_filters(query.as_deref(), page, &filters)?;
		let request = url.request()?;
		let manga_page_result = if url.is_filters() {
			request.html()?.manga_page_result()?
		} else {
			request.json_owned::<search::Root>()?.into()
		};
		Ok(manga_page_result)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_page = Url::manga(&manga.key).request()?.html()?;
		if needs_details {
			manga_page.update_details(&mut manga)?;

			// 簡介收藏按鈕：Markdown 鏈接，點擊經 deep link 路由回 source 執行收藏
			if let Some(description) = favorites::decorate_description(
				&manga.key,
				manga.description.as_deref().unwrap_or_default(),
			) {
				manga.description = Some(description);
			}

			if needs_chapters {
				send_partial_result(&manga);
			} else {
				return Ok(manga);
			}
		}

		let key = manga_page.key()?;
		manga.chapters = Url::chapter_list(&manga.key)
			.request()?
			.header("dnts", manga_page.dnt().as_deref().unwrap_or("2"))
			.json_owned::<chapter_list::Root>()?
			.chapters(&key)?;

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Url::chapter(&manga.key, &chapter.key)
			.request()?
			.html()?
			.pages()
	}
}

/// 解析簡介收藏按鈕的 deep link：`/__fav/{add|remove}/{path_word}`。
/// App 传入形态为 "https:host/path"（无 //，NSURL.resourceSpecifier 拼接），
/// 也兼容完整 "https://host/path"；先取动作段、再取漫画 ID 段。
fn parse_fav_deep_link(url: &str) -> Option<(String, bool)> {
	let rest = url.split("/__fav/").nth(1)?;
	let (action, remainder) = rest.split_once('/')?;
	let add = match action {
		"add" => true,
		"remove" => false,
		_ => return None,
	};
	let path_word = remainder
		.split('/')
		.next()
		.unwrap_or_default()
		.split('?')
		.next()
		.unwrap_or_default();
	if path_word.is_empty() || !path_word.chars().all(|c| c.is_ascii_alphanumeric()) {
		return None;
	}
	Some((String::from(path_word), add))
}

impl DeepLinkHandler for Copymanga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// 簡介收藏按鈕：/__fav/{add|remove}/{path_word}
		if let Some((path_word, add)) = parse_fav_deep_link(&url) {
			return favorites::deep_link_favorite(&path_word, add);
		}

		let mut splits = url.split('/').skip(3);
		let deep_link_result = match splits.next() {
			Some("comic") => match (splits.next(), splits.next(), splits.next()) {
				(Some(key), None, None) => Some(DeepLinkResult::Manga { key: key.into() }),
				(Some(manga_key), Some("chapter"), Some(key)) => Some(DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: key.into(),
				}),
				_ => None,
			},

			Some("h5") => match (splits.next(), splits.next(), splits.next()) {
				(Some("details"), Some("comic"), Some(key)) => {
					Some(DeepLinkResult::Manga { key: key.into() })
				}
				(Some("comicContent"), Some(manga_key), Some(key)) => {
					Some(DeepLinkResult::Chapter {
						manga_key: manga_key.into(),
						key: key.into(),
					})
				}
				_ => None,
			},

			_ => None,
		};
		Ok(deep_link_result)
	}
}

impl DynamicFilters for Copymanga {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let genre = Url::GenresPage.request()?.html()?.filter()?.into();
		Ok([genre].into())
	}
}

impl ListingProvider for Copymanga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"f:fav" => favorites::collect_page(page),
			_ => Err(error!("未知的列表: {}", listing.name)),
		}
	}
}

impl BasicLoginHandler for Copymanga {
	fn handle_basic_login(&self, key: String, username: String, password: String) -> Result<bool> {
		if key != "login" {
			bail!("登錄入口無效");
		}
		match auth::login(&username, &password) {
			Ok(_) => {
				println!("copymanga: login succeeded");
				auth::set_just_logged_in();
				Ok(true)
			}
			Err(err) => {
				println!("copymanga: login failed ({err:?})");
				Ok(false)
			}
		}
	}
}

impl NotificationHandler for Copymanga {
	fn handle_notification(&self, notification: String) {
		if notification == "login" {
			// 登录时 App 先调 handle_basic_login（置 justLoggedIn），随后才发通知；
			// 登出时没有 handle_basic_login，直接清掉本地 token。
			if auth::take_just_logged_in() {
				auth::clear_just_logged_in();
			} else {
				auth::clear_auth();
			}
		}
	}
}

register_source!(
	Copymanga,
	DeepLinkHandler,
	DynamicFilters,
	ListingProvider,
	BasicLoginHandler,
	NotificationHandler,
	Home
);
