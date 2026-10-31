#![no_std]
extern crate alloc;

mod models;
mod settings;

use aidoku::{
	AidokuError, BaseUrlProvider, BasicLoginHandler, Chapter, DeepLinkHandler, DeepLinkResult,
	DynamicFilters, Filter, FilterValue, ImageRequestProvider, Manga, MangaPageResult,
	MultiSelectFilter, Page, PageContext, Result, SelectFilter, SortFilter, Source,
	alloc::{String, Vec, format},
	helpers::uri::QueryParameters,
	imports::net::{Request, Response},
	imports::std::send_partial_result,
	prelude::*,
};
use alloc::borrow::Cow;
use alloc::string::ToString;
use alloc::vec;
use serde_json::json;

struct Sumidoku;

impl Sumidoku {
	fn login(&self, username: &str, password: &str) -> Result<bool> {
		let base_url = settings::get_base_url()?;
		let body = json!({ "username": username, "password": password }).to_string();
		let resp = Request::post(format!("{base_url}/api/v1/auth/login"))?
			.header("Content-Type", "application/json")
			.header("Accept", "application/json")
			.body(body)
			.send()?;
		if resp.status_code() != 200 {
			return Ok(false);
		}
		let login: models::LoginResponse = resp.get_json_owned()?;
		settings::set_api_token(&login.token);
		Ok(true)
	}

	fn build_get(&self, path: &str) -> Result<Request> {
		let base_url = settings::get_base_url()?;
		let mut req = Request::get(format!("{base_url}{path}"))?;
		req.set_header("Accept", "application/json");
		if let Some(token) = settings::get_api_token() {
			req.set_header("Authorization", &format!("Bearer {token}"));
		}
		Ok(req)
	}

	fn api_get(&self, path: &str) -> Result<Response> {
		let resp = self.build_get(path)?.send()?;
		if resp.status_code() == 401
			&& let Some((user, pass)) = settings::get_credentials()
			&& self.login(&user, &pass)?
		{
			return Self::check_response(self.build_get(path)?.send()?);
		}
		Self::check_response(resp)
	}

	fn check_response(resp: Response) -> Result<Response> {
		let status = resp.status_code();
		if !(200..300).contains(&status) {
			let body = resp
				.get_string()
				.unwrap_or_else(|_| String::from("<empty>"));
			bail!("HTTP {status}: {body}");
		}
		Ok(resp)
	}

	fn fetch_library(&self, qs: QueryParameters) -> Result<MangaPageResult> {
		let base_url = settings::get_base_url()?;
		let path = format!("/api/v1/library?{}", qs);
		let resp = self.api_get(&path)?;
		let page: models::LibraryPage = resp.get_json_owned()?;
		Ok(MangaPageResult {
			entries: page
				.entries
				.into_iter()
				.map(|e| e.manga.into_listing(&base_url))
				.collect(),
			has_next_page: page.has_next_page,
		})
	}
}

impl Source for Sumidoku {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));

		if let Some(q) = query {
			qs.push("search", Some(&q));
		}

		let mut sort = "added";
		let mut order = "desc";
		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					sort = match index {
						0 => "added",
						1 => "title",
						2 => "lastRead",
						3 => "latestChapter",
						4 => "unread",
						_ => sort,
					};
					order = if ascending { "asc" } else { "desc" };
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					let (include_key, exclude_key) = match id.as_str() {
						"contentRating" => ("contentRating", "contentRatingExclude"),
						"status" => ("filter", "filterExclude"),
						"source" => ("source", "sourceExclude"),
						_ => continue,
					};
					if !included.is_empty() {
						qs.push(include_key, Some(&included.join(",")));
					}
					if !excluded.is_empty() {
						qs.push(exclude_key, Some(&excluded.join(",")));
					}
				}
				FilterValue::Select { id, value } if id == "categoryId" && !value.is_empty() => {
					qs.push("categoryId", Some(&value));
				}
				FilterValue::Select { id, value } if id == "genre" && !value.is_empty() => {
					qs.push("tag", Some(&value));
				}
				FilterValue::Text { id, value } if id == "author" && !value.is_empty() => {
					qs.push("author", Some(&value));
				}
				_ => {}
			}
		}
		qs.push("sort", Some(sort));
		qs.push("order", Some(order));

		self.fetch_library(qs)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_id = manga
			.key
			.parse::<i64>()
			.map_err(|_| AidokuError::DeserializeError)?;
		let base_url = settings::get_base_url()?;

		if needs_details {
			let resp = self.api_get(&format!("/api/v1/manga/{manga_id}"))?;
			let details: models::SumiManga = resp.get_json_owned()?;
			manga = details.into_manga(&base_url);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let resp = self.api_get(&format!("/api/v1/manga/{manga_id}/chapters"))?;
			let chapters: Vec<models::SumiChapter> = resp.get_json_owned()?;
			manga.chapters = Some(
				chapters
					.into_iter()
					.map(|c| c.into_chapter(&base_url))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_id = chapter
			.key
			.parse::<i64>()
			.map_err(|_| AidokuError::DeserializeError)?;
		let resp = self.api_get(&format!("/api/v1/chapter/{chapter_id}/pages"))?;
		let pages: Vec<models::SumiPage> = resp.get_json_owned()?;
		let base_url = settings::get_base_url()?;
		Ok(pages
			.into_iter()
			.map(|p| p.into_page(&base_url, chapter_id))
			.collect())
	}
}

impl BaseUrlProvider for Sumidoku {
	fn get_base_url(&self) -> Result<String> {
		settings::get_base_url()
	}
}

impl DynamicFilters for Sumidoku {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut filters: Vec<Filter> = Vec::new();
		let [categories_res, sources_res, counts_res] = Request::send_all(vec![
			self.build_get("/api/v1/categories")?,
			self.build_get("/api/v1/sources")?,
			self.build_get("/api/v1/library/source-counts")?,
		])
		.try_into()
		.expect("requests length should be 3");

		// 1. Category
		let mut category_options: Vec<Cow<'static, str>> =
			vec!["All".into(), "Uncategorized".into()];
		let mut category_values: Vec<Cow<'static, str>> = vec!["".into(), "0".into()];
		if let Ok(resp) = categories_res
			&& resp.status_code() == 200
			&& let Ok(categories) = resp.get_json_owned::<Vec<models::SumiCategory>>()
		{
			for category in categories.into_iter().filter(|c| c.manga_count > 0) {
				category_options.push(category.name.into());
				category_values.push(category.id.to_string().into());
			}
		}
		filters.push(
			SelectFilter {
				id: "categoryId".into(),
				title: Some("Category".into()),
				options: category_options,
				ids: Some(category_values),
				..Default::default()
			}
			.into(),
		);

		// 2. Source
		let in_library_sources = match counts_res {
			Ok(resp) if resp.status_code() == 200 => resp
				.get_json_owned::<Vec<models::SourceCount>>()
				.ok()
				.map(|counts| {
					counts
						.into_iter()
						.map(|c| c.source_id)
						.collect::<Vec<i64>>()
				}),
			_ => None,
		};
		if let Ok(resp) = sources_res
			&& resp.status_code() == 200
			&& let Ok(sources) = resp.get_json_owned::<Vec<models::SumiSource>>()
		{
			let (source_names, source_ids): (Vec<_>, Vec<_>) = sources
				.into_iter()
				.filter(|s| match &in_library_sources {
					Some(ids) => ids.contains(&s.id),
					None => true,
				})
				.map(|s| (s.name.into(), s.id.to_string().into()))
				.unzip();
			if !source_ids.is_empty() {
				filters.push(
					MultiSelectFilter {
						id: "source".into(),
						title: Some("Source".into()),
						can_exclude: true,
						options: source_names,
						ids: Some(source_ids),
						..Default::default()
					}
					.into(),
				);
			}
		}

		// 3. Status
		let status_filter = MultiSelectFilter {
			id: "status".into(),
			title: Some("Status".into()),
			can_exclude: true,
			options: vec!["Unread".into(), "Downloaded".into(), "Bookmarked".into()],
			ids: Some(vec![
				"unread".into(),
				"downloaded".into(),
				"bookmarked".into(),
			]),
			..Default::default()
		};
		filters.push(status_filter.into());

		// 4. Content Rating
		let content_rating_filter = MultiSelectFilter {
			id: "contentRating".into(),
			title: Some("Content Rating".into()),
			can_exclude: true,
			options: vec!["Safe".into(), "Suggestive".into(), "NSFW".into()],
			ids: Some(vec!["1".into(), "2".into(), "3".into()]),
			..Default::default()
		};
		filters.push(content_rating_filter.into());

		// 5. Sort
		let sort_filter = SortFilter {
			id: "sort".into(),
			title: Some("Sort".into()),
			can_ascend: true,
			options: vec![
				"Added".into(),
				"Title".into(),
				"Last Read".into(),
				"Latest Chapter".into(),
				"Unread".into(),
			],
			..Default::default()
		};
		filters.push(sort_filter.into());

		Ok(filters)
	}
}

impl BasicLoginHandler for Sumidoku {
	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		self.login(&username, &password)
	}
}

impl DeepLinkHandler for Sumidoku {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let base_url = settings::get_base_url()?;
		let Some(path) = url.strip_prefix(&base_url) else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();

		if let Some(id) = path.strip_prefix("/manga/") {
			return Ok(Some(DeepLinkResult::Manga {
				key: String::from(id),
			}));
		}

		Ok(None)
	}
}

impl ImageRequestProvider for Sumidoku {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		let mut req = Request::get(&url)?;
		let base_url = settings::get_base_url()?;
		if url.starts_with(&base_url)
			&& let Some(token) = settings::get_api_token()
		{
			req.set_header("Authorization", &format!("Bearer {token}"));
		}
		if let Some(ctx) = context {
			for (key, value) in ctx {
				req.set_header(&key, &value);
			}
		}
		Ok(req)
	}
}

register_source!(
	Sumidoku,
	BaseUrlProvider,
	DynamicFilters,
	BasicLoginHandler,
	DeepLinkHandler,
	ImageRequestProvider
);
