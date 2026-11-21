#![no_std]
mod helpers;
mod home;
mod settings;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec},
	helpers::uri::QueryParameters,
	prelude::*,
};
use helpers::{
	SORTS, get_html, manga_key, manga_url, parse_chapters, parse_manga, parse_page, search_url,
	sorted_url,
};

struct VyManga;

impl Source for VyManga {
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
		if query.is_some() {
			qs.push("q", query.as_deref());
		}
		let mut included_genres = Vec::new();
		let mut excluded_genres = Vec::new();
		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					if let Some(sort) = SORTS.get(index as usize) {
						qs.push_encoded("sort", Some(sort));
					}
					if ascending {
						qs.push_encoded("sort_type", Some("asc"));
					}
				}
				FilterValue::Check { id, value } if id == "description" && value == 1 => {
					qs.push_encoded("check_search_desc", Some("1"));
				}
				FilterValue::Text { id, value } if id == "author" => {
					qs.push("author", Some(&value));
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"author_match" => qs.push_encoded("author_po", Some(&value)),
					"status" => qs.push_encoded("completed", Some(&value)),
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genre" => {
					included_genres = included;
					excluded_genres = excluded;
				}
				_ => {}
			}
		}
		let url = search_url(qs, page, &included_genres, &excluded_genres);
		Ok(parse_page(&get_html(&url)?))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let html = get_html(&manga_url(&manga.key))?;
		if needs_details {
			let Some(info) = html.select_first(".div-manga") else {
				bail!("Missing series details");
			};
			manga = parse_manga(manga.key, &info)?;
		}
		if needs_chapters {
			manga.chapters = Some(parse_chapters(&html));
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		// A chapter's link expires within hours, so a fresh one is read from the series page.
		let Some(url) = parse_chapters(&get_html(&manga_url(&manga.key))?)
			.into_iter()
			.find(|listed| listed.key == chapter.key)
			.and_then(|listed| listed.url)
		else {
			bail!("Chapter not found");
		};
		let html = get_html(&url)?;
		let Some(images) = html
			.select(".carousel-item > img")
			.filter(|images| !images.is_empty())
		else {
			bail!("No pages found");
		};
		images
			.map(|image| {
				let url = image
					.attr("data-src")
					.ok_or_else(|| error!("Missing page image"))?;
				Ok(Page {
					content: PageContent::url(url),
					..Default::default()
				})
			})
			.collect()
	}
}

impl ListingProvider for VyManga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if !SORTS.contains(&listing.id.as_str()) {
			bail!("Unknown listing");
		}
		Ok(parse_page(&get_html(&sorted_url(&listing.id, page))?))
	}
}

impl DeepLinkHandler for VyManga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(manga_key(&url).map(|key| DeepLinkResult::Manga { key }))
	}
}

register_source!(VyManga, Home, ListingProvider, DeepLinkHandler);
