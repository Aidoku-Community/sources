use crate::auth::AuthedRequest;
use crate::models::{
	Branch, CatalogResponse, ChapterPages, ChaptersResponse, SearchResponse, TitleBranches,
	TitleCard, TitleDetail,
};
use crate::settings::API_V2;
use aidoku::helpers::uri::{QueryParameters, encode_uri_component};
use aidoku::imports::net::Request;
use aidoku::{Chapter, FilterValue, Manga, MangaPageResult, Page, PageContent, Result, prelude::*};
use alloc::{format, string::String, string::ToString, vec::Vec};

const PAGE_SIZE: i32 = 30;
const CHAPTER_PAGE_SIZE: i32 = 50;

/// Filter sort indices — keep in sync with `res/filters.json`.
const ORDER_FIELDS: [&str; 7] = [
	"chapter_date",
	"avg_rating",
	"score",
	"views",
	"votes",
	"count_chapters",
	"id",
];

/// Catalog search or filtered browse used by Aidoku’s search screen.
pub fn search(
	query: Option<String>,
	page: i32,
	filters: Vec<FilterValue>,
) -> Result<MangaPageResult> {
	if let Some(q) = query.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
		return search_query(q, page);
	}
	catalog(page, filters)
}

fn search_query(query: &str, page: i32) -> Result<MangaPageResult> {
	let mut params = QueryParameters::new();
	params.push("query", Some(query));
	params.push("page", Some(&page.to_string()));
	params.push("count", Some(&PAGE_SIZE.to_string()));

	let url = format!("{API_V2}/search/?{params}");
	let response = Request::get(url)?
		.remanga()
		.json_owned::<SearchResponse>()?;
	let entries = response
		.results
		.unwrap_or_default()
		.into_iter()
		.filter_map(TitleCard::into_manga)
		.collect::<Vec<_>>();
	let has_next_page = response
		.meta
		.and_then(|m| match (m.page, m.total_pages) {
			(Some(p), Some(total)) => Some(p < total),
			_ => None,
		})
		.unwrap_or(entries.len() as i32 >= PAGE_SIZE);

	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

fn catalog(page: i32, filters: Vec<FilterValue>) -> Result<MangaPageResult> {
	let mut params = QueryParameters::new();
	params.push("page", Some(&page.to_string()));
	params.push("count", Some(&PAGE_SIZE.to_string()));

	// Default: newest chapter updates, not rating tops.
	let mut ordering = String::from("-chapter_date");
	for filter in filters {
		match filter {
			FilterValue::Sort {
				index, ascending, ..
			} => {
				let field = ORDER_FIELDS
					.get(index as usize)
					.copied()
					.unwrap_or("chapter_date");
				ordering = if ascending {
					field.into()
				} else {
					format!("-{field}")
				};
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} => match id.as_str() {
				"types" => {
					for value in included {
						params.push("types", Some(&value));
					}
				}
				"status" => {
					for value in included {
						params.push("status", Some(&value));
					}
				}
				"age_limit" => {
					for value in included {
						params.push("age_limit", Some(&value));
					}
				}
				"genres" => {
					for value in included {
						params.push("genres", Some(&value));
					}
					for value in excluded {
						params.push("exclude_genres", Some(&value));
					}
				}
				"categories" => {
					for value in included {
						params.push("categories", Some(&value));
					}
					for value in excluded {
						params.push("exclude_categories", Some(&value));
					}
				}
				_ => {}
			},
			_ => {}
		}
	}
	params.push("ordering", Some(&ordering));

	let url = format!("{API_V2}/search/catalog/?{params}");
	let response = Request::get(url)?
		.remanga()
		.json_owned::<CatalogResponse>()?;
	let entries = response
		.results
		.unwrap_or_default()
		.into_iter()
		.filter_map(TitleCard::into_manga)
		.collect::<Vec<_>>();
	let has_next_page =
		response.next.as_ref().is_some_and(|n| n.has_more()) || entries.len() as i32 >= PAGE_SIZE;

	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

fn fetch_title(slug: &str) -> Result<TitleDetail> {
	let url = format!("{API_V2}/titles/{}/", encode_uri_component(slug));
	Request::get(url)?.remanga().json_owned::<TitleDetail>()
}

/// Loads title details once and returns branches for chapter fetching (avoids a second title GET).
pub fn fetch_manga_with_branches(existing: Manga) -> (Manga, Option<Vec<Branch>>) {
	match fetch_title(&existing.key) {
		Ok(detail) => {
			let branches = detail.branches().to_vec();
			let branches = if branches.is_empty() {
				None
			} else {
				Some(branches)
			};
			(detail.into_manga(Some(existing)), branches)
		}
		Err(_) => (existing, None),
	}
}

fn fetch_branches(slug: &str) -> Result<Vec<Branch>> {
	let url = format!("{API_V2}/titles/{}/", encode_uri_component(slug));
	if let Ok(title) = Request::get(&url)?.remanga().json_owned::<TitleBranches>()
		&& let Some(branches) = title.branches.filter(|b| !b.is_empty())
	{
		return Ok(branches);
	}
	let detail = fetch_title(slug)?;
	let branches = detail.branches().to_vec();
	if branches.is_empty() {
		bail!("У тайтла нет веток перевода");
	}
	Ok(branches)
}

fn select_primary_branch(mut branches: Vec<Branch>) -> Result<Branch> {
	branches.sort_by(|a, b| {
		b.count_chapters
			.unwrap_or(0)
			.cmp(&a.count_chapters.unwrap_or(0))
			.then_with(|| b.id.cmp(&a.id))
	});
	branches
		.into_iter()
		.next()
		.ok_or_else(|| error!("У тайтла нет веток перевода"))
}

/// Fetches chapters for the primary branch.
///
/// `branches` may come from a prior title details call to skip an extra request.
/// `on_progress` is invoked after each page so Aidoku can show a growing list.
pub fn fetch_chapters(
	slug: &str,
	branches: Option<Vec<Branch>>,
	mut on_progress: impl FnMut(&[Chapter]),
) -> Result<Vec<Chapter>> {
	let branches = match branches {
		Some(b) if !b.is_empty() => b,
		_ => fetch_branches(slug)?,
	};
	let branch = select_primary_branch(branches)?;
	let label = branch
		.publishers
		.and_then(|p| p.into_iter().next())
		.and_then(|p| p.name);

	let mut chapters = Vec::new();
	let mut page = 1;
	loop {
		let mut params = QueryParameters::new();
		params.push("branch_id", Some(&branch.id.to_string()));
		params.push("ordering", Some("-index"));
		params.push("user_data", Some("1"));
		params.push("count", Some(&CHAPTER_PAGE_SIZE.to_string()));
		params.push("page", Some(&page.to_string()));
		let url = format!("{API_V2}/titles/chapters/?{params}");
		let response = Request::get(url)?
			.remanga()
			.json_owned::<ChaptersResponse>()?;
		let batch = response.results.unwrap_or_default();
		if batch.is_empty() {
			break;
		}
		let has_more = response.next.as_ref().is_some_and(|n| n.has_more());
		for item in batch {
			chapters.push(item.into_chapter(slug, label.as_deref()));
		}
		on_progress(&chapters);
		if !has_more {
			break;
		}
		page += 1;
		if page > 400 {
			break;
		}
	}

	Ok(chapters)
}

fn absolutize_page_url(link: &str, server: Option<&str>) -> String {
	if link.starts_with("http://") || link.starts_with("https://") {
		return link.into();
	}
	if let Some(base) = server.filter(|s| !s.is_empty()) {
		let base = base.trim_end_matches('/');
		if link.starts_with('/') {
			format!("{base}{link}")
		} else {
			format!("{base}/{link}")
		}
	} else {
		crate::settings::media_url(link)
	}
}

/// Loads page image URLs for a chapter id (requires purchase when the chapter is paid).
pub fn fetch_pages(chapter_id: &str) -> Result<Vec<Page>> {
	let url = format!("{API_V2}/titles/chapters/{chapter_id}/");
	let response = Request::get(url)?.remanga().json_owned::<ChapterPages>()?;

	let paid = response.is_paid.unwrap_or(false);
	let bought = response.is_bought.unwrap_or(false);
	let server = response
		.server
		.as_ref()
		.and_then(|s| s.link.as_deref().or(s.fallback_link.as_deref()))
		.map(String::from);

	let pages = response
		.pages
		.map(|field| {
			field
				.flatten()
				.into_iter()
				.filter_map(|p| p.link)
				.filter(|l| !l.is_empty())
				.map(|l| absolutize_page_url(&l, server.as_deref()))
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();

	if pages.is_empty() {
		if paid && !bought {
			bail!("Глава платная. Войдите с покупкой или дождитесь бесплатной публикации.");
		}
		bail!("Страницы главы недоступны");
	}

	Ok(pages
		.into_iter()
		.map(|url| Page {
			content: PageContent::url(url),
			..Default::default()
		})
		.collect())
}
