#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};
use serde::de::DeserializeOwned;

mod models;
use models::*;

const BASE_URL: &str = "https://okutoon.net";
const API_URL: &str = "https://okutoon.net/api/v1";

/// Site `sort` values, in the order of the sort filter options.
const SORT_IDS: &[&str] = &["newest", "updated", "popular", "rating", "az"];

/// The chapter endpoint returns at most 50 chapters per request.
const CHAPTER_PAGE_SIZE: i32 = 50;

const PAGE_ERROR: &str = "Bölüm sayfaları alınamadı.";
const NETWORK_ERROR: &str = "Sayfa okutoon.net'ten alınamadı.";

/// Top-level site paths that aren't series slugs.
const NON_SERIES_PATHS: &[&str] = &[
	"tur",
	"arama",
	"giris",
	"kayit",
	"login",
	"register",
	"profil",
	"forum",
	"sayfa",
	"yorumlar",
	"images",
	"kutuphane",
	"hakkimizda",
	"iletisim",
	"gizlilik",
	"api",
];

fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.json_owned::<T>()
		.map_err(|_| error!("{NETWORK_ERROR}"))
}

struct OkuToon;

impl Source for OkuToon {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut genre = None;
		let mut sort = SORT_IDS[0];
		let mut period = None;
		for filter in &filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					sort = SORT_IDS.get(*index as usize).copied().unwrap_or(sort);
				}
				FilterValue::Select { id, value } => match id.as_str() {
					// the "Tümü" option has an empty id
					"genre" => genre = Some(value.as_str()).filter(|g| !g.is_empty()),
					"period" => period = Some(value.as_str()),
					_ => {}
				},
				_ => {}
			}
		}

		let mut qs = QueryParameters::new();
		qs.push("sort", Some(sort));
		if sort == "popular" && period.is_some() {
			qs.push("period", period);
		}
		if query.is_some() {
			qs.push("q", query.as_deref());
		}
		if genre.is_some() {
			qs.push("categories", genre);
		}
		qs.push("page", Some(&page.to_string()));
		Ok(get_json::<SeriesList>(&format!("{API_URL}/series?{qs}"))?.into())
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let series: Response<Series> = get_json(&format!("{API_URL}/series/{}", manga.key))?;
			manga.copy_from(series.data.into());
		}
		if needs_chapters {
			let mut chapters = Vec::new();
			let mut offset = 0;
			loop {
				let list: ChapterList = get_json(&format!(
					"{API_URL}/series/{}/chapters?offset={offset}&limit={CHAPTER_PAGE_SIZE}",
					manga.key
				))?;
				chapters.extend(list.data.into_iter().map(Chapter::from));
				if !list.meta.has_more {
					break;
				}
				offset = list.meta.offset;
			}
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let reader: Response<ChapterPages> = get_json(&format!(
			"{API_URL}/series/{}/chapters/{}",
			manga.key, chapter.key
		))?;
		if reader.data.pages.is_empty() {
			bail!("{PAGE_ERROR}");
		}
		Ok(reader
			.data
			.pages
			.into_iter()
			.map(|page| Page {
				content: PageContent::Url(page.image_url, None),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for OkuToon {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		// listing ids match the sort ids; the popular listing is weekly
		let index = SORT_IDS
			.iter()
			.position(|id| *id == listing.id)
			.unwrap_or_default() as i32;
		let mut filters = vec![FilterValue::Sort {
			id: "sort".into(),
			index,
			ascending: false,
		}];
		if listing.id == "popular" {
			filters.push(FilterValue::Select {
				id: "period".into(),
				value: "weekly".into(),
			});
		}
		self.get_search_manga_list(None, page, filters)
	}
}

impl DeepLinkHandler for OkuToon {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.strip_prefix(BASE_URL)
			.unwrap_or_default()
			.split(['?', '#'])
			.next()
			.unwrap_or_default()
			.trim_matches('/');
		let mut segments = path.split('/');
		let Some(series) = segments.next().filter(|s| !s.is_empty()) else {
			return Ok(None);
		};
		// `/@<user>` paths are user profiles
		if series.starts_with('@') || NON_SERIES_PATHS.contains(&series) {
			return Ok(None);
		}
		// chapter slugs are always `bolum-*`; other sub-pages (the series
		// library, follower lists, ...) open the series instead
		Ok(Some(
			match segments.next().filter(|s| s.starts_with("bolum")) {
				Some(chapter) => DeepLinkResult::Chapter {
					manga_key: series.to_string(),
					key: chapter.to_string(),
				},
				_ => DeepLinkResult::Manga {
					key: series.to_string(),
				},
			},
		))
	}
}

register_source!(OkuToon, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod tests;
