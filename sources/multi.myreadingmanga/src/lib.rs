#![no_std]

extern crate alloc;

mod helpers;
mod parser;

use crate::helpers::{BASE_URL, HTTP_URL, UA, get, get_user_languages, page_url};
use crate::parser::{parse_chapters, parse_listing, parse_manga, parse_pages};

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue,
	ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContext,
	Result, SelectFilter, Source, bail,
	helpers::uri::encode_uri_component,
	imports::net::{Request, TimeUnit},
	prelude::*,
	register_source,
};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

struct MyReadingManga;

impl Source for MyReadingManga {
	fn new() -> Self {
		aidoku::imports::net::set_rate_limit(1, 2, TimeUnit::Seconds);
		MyReadingManga
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> aidoku::imports::error::Result<MangaPageResult> {
		let mut query_str = alloc::format!(
			"?s={}",
			encode_uri_component(query.as_deref().unwrap_or(""))
		);
		let mut sort_param = "date";

		for filter in &filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					sort_param = match index {
						0 if *ascending => "date_asc",
						0 => "date",
						1 => "relevance",
						2 => "rand",
						_ => "date",
					};
				}
				FilterValue::Select { id, value } if !value.is_empty() => {
					let param = match id.as_str() {
						"status" => "ep_filter_status",
						"genre" => "ep_filter_genre",
						"category" => "ep_filter_category",
						"tag" => "ep_filter_post_tag",
						"artist" => "ep_filter_artist",
						"pairing" => "ep_filter_pairing",
						_ => continue,
					};
					query_str.push_str(&format!("&{}={}", param, value));
				}
				FilterValue::Text { id, value } if id == "tag" && !value.is_empty() => {
					let slug = encode_uri_component(value.trim().to_lowercase().replace(' ', "-"));
					query_str.push_str(&format!("&ep_filter_post_tag={}", slug));
				}
				_ => {}
			}
		}

		query_str.push_str(&format!("&ep_sort={}", sort_param));

		let url = page_url(&format!("{}{}", BASE_URL, query_str), page);
		let doc = get(&url)?;
		let (entries, has_next_page) = parse_listing(&doc, &get_user_languages());

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut manga = manga;
		let url = format!("{}/{}/", BASE_URL, manga.key);
		let doc = get(&url)?;

		if needs_details {
			parse_manga(&doc, &mut manga);
		}

		if needs_chapters {
			manga.chapters = Some(parse_chapters(&doc, &manga.key));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{}/{}/", BASE_URL, chapter.key);
		let doc = get(&url)?;
		let pages = parse_pages(&doc);

		if pages.is_empty() {
			bail!("No pages found. MRM may be down or showing a Cloudflare alert!!!");
		}

		Ok(pages)
	}
}

impl ListingProvider for MyReadingManga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let base = match listing.name.as_str() {
			"Popular" => format!("{}/popular", BASE_URL),
			"Manga" => format!("{}/yaoi-manga", BASE_URL),
			"Bara" => format!("{}/genre/bara", BASE_URL),
			"Random" => format!("{}/?ep_sort=rand&s=", BASE_URL),
			_ => BASE_URL.to_string(),
		};

		let url = page_url(&base, page);
		let doc = get(&url)?;
		let (entries, has_next_page) = parse_listing(&doc, &get_user_languages());

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

impl DeepLinkHandler for MyReadingManga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let key = url
			.trim_start_matches(BASE_URL)
			.trim_end_matches('/')
			.to_string();
		Ok(Some(DeepLinkResult::Manga { key }))
	}
}

impl ImageRequestProvider for MyReadingManga {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("User-Agent", UA)
			.header("Referer", BASE_URL))
	}
}

impl DynamicFilters for MyReadingManga {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut dynamic_filters: Vec<Filter> = Vec::new();

		let doc = get(&format!("{}/?ep_sort=rand&s=", BASE_URL))?;

		let Some(sidebar) = doc.select_first("aside.ep-search-sidebar") else {
			return Ok(dynamic_filters);
		};

		let Some(widgets) = sidebar.select("div.ep-filter-widget") else {
			return Ok(dynamic_filters);
		};

		for widget in widgets {
			let Some(title) = widget
				.select_first("h3.ep-filter-title")
				.and_then(|e| e.text())
			else {
				continue;
			};

			let filter_id = match title.to_lowercase().as_str() {
				"genre" => "genre",
				"category" => "category",
				"tag" => "tag",
				"circle/ artist" => "artist",
				"pairing" => "pairing",
				"status" => "status",
				_ => continue,
			};

			let mut options = vec![String::from("Any")];
			let mut values = vec![String::from("")];

			if let Some(terms) = widget.select("div.term") {
				for term in terms {
					let name = term.attr("data-term-name").unwrap_or_default();
					let slug = term.attr("data-term-slug").unwrap_or_default();
					if !name.is_empty() && !slug.is_empty() {
						options.push(name);
						values.push(slug);
					}
				}
			}

			if options.len() > 1 {
				dynamic_filters.push(
					SelectFilter {
						id: filter_id.into(),
						title: Some(title.into()),
						options: options.into_iter().map(Into::into).collect(),
						ids: Some(values.into_iter().map(Into::into).collect()),
						..Default::default()
					}
					.into(),
				);
			}
		}

		Ok(dynamic_filters)
	}
}

register_source!(
	MyReadingManga,
	DeepLinkHandler,
	ImageRequestProvider,
	ListingProvider,
	DynamicFilters
);
