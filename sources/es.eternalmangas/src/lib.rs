#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, ImageRequestProvider,
	Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};
mod helpers;
mod home;
mod markdown;
mod models;
mod settings;
const URL: &str = "https://eternalmangas.org";
struct EternalMangas;

impl Source for EternalMangas {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(key) = query
			.as_deref()
			.and_then(|query| query.strip_prefix(URL))
			.and_then(helpers::series_key)
		{
			let url = format!("{URL}{key}");
			let response = helpers::request(&url)?.send()?;
			if response.status_code() != 404 {
				let html = helpers::check_response(response, &url)?.get_html()?;
				return Ok(MangaPageResult {
					entries: vec![helpers::basic(&html, key, settings::label_novels())?],
					has_next_page: false,
				});
			}
		}
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		if let Some(query) = query {
			qs.push("search", Some(&query));
		}
		for filter in filters {
			if let FilterValue::Select { id, value } = filter
				&& matches!(id.as_str(), "type" | "status" | "genre")
			{
				qs.push(&id, Some(&value));
			}
		}
		Ok(helpers::search(
			&helpers::get_html(&format!("{URL}/series?{qs}"))?,
			settings::label_novels(),
		))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let key = helpers::resolve_series_key(&manga.key)?;
		let html = helpers::get_html(&format!("{URL}{key}"))?;
		if needs_details {
			let mut details = helpers::details(&html, key, settings::label_novels())?;
			details.key = core::mem::take(&mut manga.key);
			manga.copy_from(details);
		}
		if needs_chapters {
			manga.chapters = Some(helpers::chapters(&html)?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let key = if chapter.key.parse::<i32>().is_ok() {
			let series = helpers::resolve_series_key(&manga.key)?;
			let url = format!(
				"https://api.eternalmangas.org/api/chapter?chapterId={}",
				chapter.key
			);
			let data: models::LegacyChapterResponse =
				helpers::check_response(helpers::request(&url)?.send()?, &url)?.get_json_owned()?;
			format!("{series}/{}", data.chapter.slug)
		} else {
			chapter.key
		};
		helpers::pages(&helpers::get_html(&format!("{URL}{key}"))?)
	}
}

impl Home for EternalMangas {
	fn get_home(&self) -> Result<HomeLayout> {
		home::parse_home(&helpers::get_html(URL)?)
	}
}

impl ImageRequestProvider for EternalMangas {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{URL}/")))
	}
}

impl DeepLinkHandler for EternalMangas {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.splitn(4, '/').nth(3).map(|path| format!("/{path}")) else {
			return Ok(None);
		};
		let Some(manga_key) = helpers::series_key(&path) else {
			return Ok(None);
		};
		let path = path
			.split(['?', '#'])
			.next()
			.unwrap_or(&path)
			.trim_end_matches('/');
		Ok(Some(if path == manga_key {
			DeepLinkResult::Manga { key: manga_key }
		} else {
			DeepLinkResult::Chapter {
				manga_key,
				key: path.into(),
			}
		}))
	}
}

register_source!(EternalMangas, Home, DeepLinkHandler, ImageRequestProvider);
