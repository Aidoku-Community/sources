#![no_std]

mod html;
mod json;
mod net;
mod setting;

use aidoku::{
	Chapter, DynamicFilters, Filter, FilterValue, HashMap, Manga, MangaPageResult,
	NotificationHandler, Page, Result, Source, WebLoginHandler,
	alloc::{String, Vec},
	bail, register_source,
};
use html::{FiltersPage as _, MangaPage as _};
use json::manga_page_result;
use net::Url;
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
		let manga_page_result = Url::from_query_or_filters(query.as_deref(), page, &filters)?
			.request()?
			.json_owned::<manga_page_result::Root>()?
			.into();
		Ok(manga_page_result)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if !needs_details && !needs_chapters {
			return Ok(manga);
		}

		let manga_page = Url::manga(&manga.key).request()?.html()?;

		if needs_details {
			let updated_details = manga_page.manga_details()?;

			manga = Manga {
				chapters: manga.chapters,
				..updated_details
			};
		}

		if needs_chapters {
			manga.chapters = manga_page.chapters()?;
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		todo!()
	}
}

impl DynamicFilters for Boylove {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let tags = Url::FiltersPage.request()?.html()?.tags_filter()?;

		let filters = [tags].into();
		Ok(filters)
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

register_source!(
	Boylove,
	DynamicFilters,
	NotificationHandler,
	WebLoginHandler
);
