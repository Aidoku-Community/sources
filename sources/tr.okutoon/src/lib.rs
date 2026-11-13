#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, Result, Source,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::encode_uri_component,
	prelude::*,
};

mod helpers;
use helpers::*;

const BASE_URL: &str = "https://okutoon.com";

const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

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
		let mut genre = "";
		let mut sort = "newest";
		let mut period = "";
		for filter in &filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"genre" => genre = value.as_str(),
					"sort" => sort = value.as_str(),
					"period" => period = value.as_str(),
					_ => {}
				}
			}
		}

		let path = if genre.is_empty() {
			"/tur".to_string()
		} else {
			format!("/tur/{genre}")
		};
		let mut params = Vec::new();
		if let Some(query) = query.filter(|q| !q.is_empty()) {
			params.push(("q", encode_uri_component(query)));
		}
		params.push(("sira", sort.to_string()));
		if sort == "popular" && !period.is_empty() {
			params.push(("donem", period.to_string()));
		}
		if page > 1 {
			params.push(("sayfa", page.to_string()));
		}
		fetch_listing(&path, params)
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
		let sort = match listing.id.as_str() {
			"popular" => "popular",
			"updated" => "updated",
			"rating" => "rating",
			_ => "newest",
		};
		let mut params = Vec::new();
		params.push(("sira", sort.to_string()));
		if sort == "popular" {
			params.push(("donem", "weekly".to_string()));
		}
		if page > 1 {
			params.push(("sayfa", page.to_string()));
		}
		fetch_listing("/tur", params)
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
