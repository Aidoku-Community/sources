use crate::auth::AuthedRequest;
use crate::models::{
	DesuChapter, DesuChapterResponse, DesuChaptersResponse, DesuError, DesuItem, DesuMangaResponse,
};
use crate::settings::{base_url, domain, rewrite_media_url};
use aidoku::helpers::uri::QueryParameters;
use aidoku::imports::{
	html::{Document, Html},
	net::Request,
};
use aidoku::{FilterValue, Manga, Result, alloc::String, error, prelude::*};
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

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
		Ok(chapters
			.into_iter()
			.filter(|chapter| chapter.is_readable.unwrap_or(true))
			.collect())
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
	pub entries: Vec<Manga>,
	pub has_next_page: bool,
}

fn manga_id_from_url(url: &str) -> Option<String> {
	url.split('?')
		.next()
		.unwrap_or(url)
		.trim_end_matches('/')
		.rsplit('/')
		.next()
		.and_then(|slug| {
			slug.rsplit_once('.')
				.map(|(_, id)| id)
				.filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
		})
		.map(String::from)
}

fn cover_from_style(style: &str) -> Option<String> {
	let start = style.find("url(")?;
	let rest = style[start + 4..].trim_start_matches(['\'', '"']);
	let end = rest.find(['\'', '"', ')'])?;
	let url = rest[..end].trim();
	(!url.is_empty()).then(|| rewrite_media_url(url))
}

fn parse_catalog_item(element: aidoku::imports::html::Element) -> Option<Manga> {
	let link = element.select_first("h3 a")?;
	let url = link.attr("abs:href")?;
	let id = manga_id_from_url(&url)?;
	let english = link.own_text()?;
	let russian = element
		.select_first(".dimmed.oTitle span")
		.and_then(|el| el.text())
		.map(|text| text.trim().into())
		.filter(|text: &String| !text.is_empty());
	let title = if crate::settings::eng_title() {
		english
	} else {
		russian.unwrap_or(english)
	};
	let cover = element
		.select_first("span.img")
		.and_then(|el| el.attr("style"))
		.and_then(|style| cover_from_style(&style));
	Some(Manga {
		key: crate::keys::manga_key(&id),
		title,
		cover,
		url: Some(url),
		..Default::default()
	})
}

fn parse_quick_search_item(element: aidoku::imports::html::Element) -> Option<Manga> {
	let link = element.select_first("a")?;
	let url = link.attr("abs:href")?;
	let id = manga_id_from_url(&url)?;
	let english = element.select_first(".itemTitle")?.text()?.trim().into();
	let russian = element
		.select_first(".itemSubTitle")
		.and_then(|el| el.text())
		.map(|text| text.trim().into())
		.filter(|text: &String| !text.is_empty());
	let title = if crate::settings::eng_title() {
		english
	} else {
		russian.unwrap_or(english)
	};
	let cover = link
		.select_first("img")
		.and_then(|el| el.attr("abs:src"))
		.map(|url| rewrite_media_url(&url));
	Some(Manga {
		key: crate::keys::manga_key(&id),
		title,
		cover,
		url: Some(url),
		..Default::default()
	})
}

fn catalog_url(page: i32, filters: Vec<FilterValue>) -> String {
	let mut params = QueryParameters::new();
	if page > 1 {
		params.push("page", Some(page.to_string().as_str()));
	}
	let mut genres = Vec::new();
	for filter in filters {
		match filter {
			FilterValue::Sort { index, .. } => {
				let order = match index {
					0 => "id",
					1 => "name",
					2 => "popular",
					_ => "updated",
				};
				if order != "updated" {
					params.push("order_by", Some(order));
				}
			}
			FilterValue::Select { id, value } if id != "section" => {
				params.push(&id, Some(&value));
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} => {
				let values: Vec<String> = included
					.into_iter()
					.chain(excluded.into_iter().map(|value| format!("!{value}")))
					.collect();
				if values.is_empty() {
					continue;
				}
				if id == "genres" || id == "tags" {
					genres.extend(values);
				} else {
					params.push(&id, Some(&values.join(",")));
				}
			}
			_ => {}
		}
	}
	if !genres.is_empty() {
		params.push("genres", Some(&genres.join(",")));
	}
	let query = params.to_string();
	if query.is_empty() {
		format!("{}/manga/", get_base_url())
	} else {
		format!("{}/manga/?{query}", get_base_url())
	}
}

fn parse_response_html(response: aidoku::imports::net::Response, base: &str) -> Result<Document> {
	if response.status_code() >= 400 {
		bail!("HTTP {}", response.status_code());
	}
	Ok(Html::parse_with_url(response.get_string()?, base)?)
}

pub fn search(query: Option<String>, page: i32, filters: Vec<FilterValue>) -> Result<SearchResult> {
	if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
		let mut params = QueryParameters::new();
		params.push("q", Some(query.as_str()));
		let response = apply_headers(
			Request::post(format!("{}/manga/search/", get_base_url()))?
				.body(params.to_string())
				.header("Content-Type", "application/x-www-form-urlencoded")
				.header("X-Requested-With", "XMLHttpRequest"),
		)
		.send()?;
		let html = parse_response_html(response, &format!("{}/manga/", get_base_url()))?;
		let entries = html
			.select("#acpQuickSearch ul.blockLinksList > li")
			.map(|elements| elements.filter_map(parse_quick_search_item).collect())
			.unwrap_or_default();
		return Ok(SearchResult {
			entries,
			has_next_page: false,
		});
	}

	let url = catalog_url(page, filters);
	let html = parse_response_html(
		apply_headers(Request::get(url.clone())?).send()?,
		&format!("{}/manga/", get_base_url()),
	)?;
	let entries = html
		.select("li.memberListItem")
		.map(|elements| elements.filter_map(parse_catalog_item).collect())
		.unwrap_or_default();
	let current_page = html
		.select_first(".PageNav")
		.and_then(|el| el.attr("data-page"))
		.and_then(|value| value.parse::<i32>().ok())
		.unwrap_or(page);
	let last_page = html
		.select_first(".PageNav")
		.and_then(|el| el.attr("data-last"))
		.and_then(|value| value.parse::<i32>().ok())
		.unwrap_or(current_page);
	Ok(SearchResult {
		entries,
		has_next_page: current_page < last_page,
	})
}
