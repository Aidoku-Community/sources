#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};

mod models;
use models::*;

const BASE_URL: &str = "https://omegascans.org";
const API_URL: &str = "https://api.omegascans.org";

struct OmegaScans;

fn get_series_list(
	page: i32,
	query: Option<&str>,
	order_by: &str,
	ascending: bool,
	status: &str,
	tag_ids: &str,
) -> Result<MangaPageResult> {
	let mut qs = QueryParameters::new();
	qs.push("query_string", Some(query.unwrap_or_default()));
	qs.push("series_type", Some("Comic"));
	qs.push("orderBy", Some(order_by));
	qs.push("order", Some(if ascending { "asc" } else { "desc" }));
	qs.push("status", Some(status));
	qs.push("tags_ids", Some(&format!("[{tag_ids}]")));
	qs.push("adult", Some("true"));
	qs.push("page", Some(&page.to_string()));
	qs.push("perPage", Some("20"));

	Ok(Request::get(format!("{API_URL}/query?{qs}"))?
		.json_owned::<Paginated<Series>>()?
		.into())
}

impl Source for OmegaScans {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut order_by = "latest";
		let mut ascending = false;
		let mut status = "All";
		let mut tag_ids = String::new();

		for filter in &filters {
			match filter {
				FilterValue::Sort {
					index,
					ascending: asc,
					..
				} => {
					order_by = match index {
						1 => "total_views",
						2 => "title",
						3 => "created_at",
						_ => "latest",
					};
					ascending = *asc;
				}
				FilterValue::Select { id, value } if id == "status" => status = value,
				FilterValue::MultiSelect { id, included, .. } if id == "tags" => {
					tag_ids = included.join(",");
				}
				_ => {}
			}
		}

		get_series_list(
			page,
			query.as_deref(),
			order_by,
			ascending,
			status,
			&tag_ids,
		)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// the series id is required to fetch chapters, so the details are always requested
		let series =
			Request::get(format!("{API_URL}/series/{}", manga.key))?.json_owned::<Series>()?;

		if needs_chapters {
			let mut chapters = Vec::new();
			let mut page = 1;
			loop {
				let response = Request::get(format!(
					"{API_URL}/chapter/query?page={page}&perPage=1000&series_id={}",
					series.id
				))?
				.json_owned::<Paginated<ChapterItem>>()?;
				let has_next_page = response.meta.has_next_page();
				chapters.extend(
					response
						.data
						.into_iter()
						.map(|chapter| chapter.into_chapter(&manga.key)),
				);
				if !has_next_page {
					break;
				}
				page += 1;
			}
			manga.chapters = Some(chapters);
		}

		if needs_details {
			manga.copy_from(series.into());
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let response = Request::get(format!("{API_URL}/chapter/{}/{}", manga.key, chapter.key))?
			.json_owned::<ChapterResponse>()?;

		let Some(data) = response.chapter.chapter_data else {
			bail!("This chapter is locked");
		};

		Ok(data
			.images
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for OmegaScans {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let order_by = match listing.id.as_str() {
			"popular" => "total_views",
			_ => "latest",
		};
		get_series_list(page, None, order_by, false, "All", "")
	}
}

impl DeepLinkHandler for OmegaScans {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(BASE_URL)
			.and_then(|path| path.strip_prefix("/series/"))
		else {
			return Ok(None);
		};

		let path = path
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.trim_end_matches('/');

		Ok(match path.split_once('/') {
			// https://omegascans.org/series/money-games/chapter-30
			Some((manga_key, key)) => Some(DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: key.into(),
			}),
			// https://omegascans.org/series/money-games
			None if !path.is_empty() => Some(DeepLinkResult::Manga { key: path.into() }),
			None => None,
		})
	}
}

register_source!(OmegaScans, ListingProvider, DeepLinkHandler);
