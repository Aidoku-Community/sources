#![no_std]

mod net;
mod setting;

use aidoku::{
	Chapter, FilterValue, HashMap, Manga, MangaPageResult, NotificationHandler, Page, Result,
	Source, WebLoginHandler,
	alloc::{String, Vec},
	bail, register_source,
};
use setting::change_charset;

struct Boylove;

impl Source for Boylove {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		todo!()
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		todo!()
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		todo!()
	}
}

impl NotificationHandler for Boylove {
	fn handle_notification(&self, notification: String) {
		if notification == "updatedCharset" {
			_ = change_charset();
		}
	}
}

impl WebLoginHandler for Boylove {
	fn handle_web_login(&self, key: String, cookies: HashMap<String, String>) -> Result<bool> {
		if key != "login" {
			bail!("Invalid login key: `{key}`");
		}

		let is_logged_in = cookies.get("rfv").is_some();
		Ok(is_logged_in)
	}
}

register_source!(Boylove, NotificationHandler, WebLoginHandler);
