#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, Result, Source,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::uri::QueryParameters,
	prelude::*,
};

mod helpers;
use helpers::*;

const BASE_URL: &str = "https://okutoon.com";

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

/// Site `sira` values, in the order of the sort filter options.
const SORT_IDS: &[&str] = &["newest", "updated", "popular", "rating", "az"];

const PAGE_ERROR: &str = "Bölüm sayfaları alınamadı.";
const NETWORK_ERROR: &str = "Sayfa okutoon.com'dan alınamadı.";

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
		if let Some(query) = query.as_deref() {
			qs.push("q", Some(query));
		}
		qs.push("sira", Some(sort));
		if sort == "popular" && period.is_some() {
			qs.push("donem", period);
		}
		if page > 1 {
			qs.push("sayfa", Some(&page.to_string()));
		}
		let url = match genre {
			Some(genre) => format!("{BASE_URL}/tur/{genre}?{qs}"),
			None => format!("{BASE_URL}/tur?{qs}"),
		};
		Ok(parse_cards(&request_document(&url)?))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let document = request_document(&format!("{BASE_URL}/{}", manga.key))?;
		if needs_details {
			manga.copy_from(parse_details(&manga.key, &document));
		}
		if needs_chapters {
			manga.chapters = Some(parse_chapters(&manga.key, &document));
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let document = request_document(&format!("{BASE_URL}/{}/{}", manga.key, chapter.key))?;
		parse_pages(&document)
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
