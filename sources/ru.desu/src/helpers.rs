use crate::auth::AuthedRequest;
use crate::models::{
	DesuChapter, DesuChapterResponse, DesuChaptersResponse, DesuError, DesuItem, DesuListResponse,
	DesuMangaResponse,
};
use crate::settings::{base_url, domain, rewrite_media_url};
use aidoku::helpers::uri::QueryParameters;
use aidoku::imports::net::Request;
use aidoku::{FilterValue, Result, alloc::String, error};
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

pub const PAGE_SIZE: i32 = 10;

pub fn get_base_url() -> String {
	base_url()
}

pub fn get_base_api_url() -> String {
	format!("https://{}/api/manga", domain())
}

pub fn apply_headers(request: Request) -> Request {
	request
		.authed()
		.header("User-Agent", "Aidoku")
		.header("Referer", get_base_url().as_str())
		.header(
			"Accept",
			"text/html,application/xhtml+xml,application/json;q=0.9,*/*;q=0.8",
		)
}

fn format_errors(errors: Option<Vec<DesuError>>) -> String {
	errors
		.map(|errs| {
			errs.into_iter()
				.filter_map(|e| e.message.or(e.code))
				.collect::<Vec<_>>()
				.join("; ")
		})
		.filter(|s| !s.is_empty())
		.unwrap_or_else(|| String::from("unknown error"))
}

pub fn fetch_by_id(id: &str) -> Result<DesuItem> {
	let url = format!("{}/{}", get_base_api_url(), id);
	let response = apply_headers(Request::get(url)?).json_owned::<DesuMangaResponse>()?;

	if let Some(res) = response.manga {
		Ok(res)
	} else {
		Err(error!(
			"Failed to fetch \"{}\": {}",
			id,
			format_errors(response.errors)
		))
	}
}

pub fn fetch_chapters(id: &str) -> Result<Vec<DesuChapter>> {
	let url = format!("{}/{}/chapters", get_base_api_url(), id);
	let response = apply_headers(Request::get(url)?).json_owned::<DesuChaptersResponse>()?;

	if let Some(chapters) = response.chapters {
		Ok(chapters)
	} else {
		Err(error!(
			"Failed to fetch chapters for \"{}\": {}",
			id,
			format_errors(response.errors)
		))
	}
}

pub fn fetch_chapter_pages(manga_id: &str, chapter_id: &str) -> Result<Vec<String>> {
	let url = format!(
		"{}/{}/chapters/{}",
		get_base_api_url(),
		manga_id,
		chapter_id
	);
	let response = apply_headers(Request::get(url)?).json_owned::<DesuChapterResponse>()?;

	if let Some(chapter) = response.chapter {
		Ok(chapter
			.pages
			.unwrap_or_default()
			.into_iter()
			.filter_map(|p| p.url.map(|url| rewrite_media_url(&url)))
			.collect())
	} else {
		Err(error!(
			"Failed to fetch pages for \"{}/{}\": {}",
			manga_id,
			chapter_id,
			format_errors(response.errors)
		))
	}
}

pub struct SearchResult {
	pub entries: Vec<DesuItem>,
	pub has_next_page: bool,
}

pub fn search(query: Option<String>, page: i32, filters: Vec<FilterValue>) -> Result<SearchResult> {
	let mut params = QueryParameters::new();
	params.push("page", Some(page.to_string().as_str()));

	if let Some(q) = query {
		params.push("search", Some(q.as_str()));
	}

	let mut order = "updated";
	let mut genres: Vec<String> = Vec::new();
	for filter in filters {
		match filter {
			FilterValue::Sort { index, .. } => {
				order = match index {
					0 => "id",
					1 => "name",
					2 => "popular",
					_ => order,
				}
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} => {
				let values: Vec<_> = included
					.into_iter()
					.chain(excluded.into_iter().map(|x| format!("!{x}")))
					.collect();
				if id.eq("genres") || id.eq("tags") {
					genres.extend(values);
				} else if !values.is_empty() {
					params.push(&id, Some(&values.join(",")));
				}
			}
			FilterValue::Select { id, value } => params.push(&id, Some(&value)),
			_ => continue,
		}
	}
	params.push("order_by", Some(order));
	if !genres.is_empty() {
		params.push("genres", Some(&genres.join(",")));
	}

	let url = format!("{}?{}", get_base_api_url(), params);
	let response = apply_headers(Request::get(url)?).json_owned::<DesuListResponse>()?;

	if let Some(mangas) = response.mangas {
		let has_next_page = response
			.pagination
			.as_ref()
			.and_then(|p| {
				let current = p.current_page?;
				let last = p.last_page?;
				Some(current < last)
			})
			.unwrap_or(mangas.len() as i32 >= PAGE_SIZE);

		Ok(SearchResult {
			entries: mangas,
			has_next_page,
		})
	} else {
		Err(error!(
			"Failed to run search: {}",
			format_errors(response.errors)
		))
	}
}
