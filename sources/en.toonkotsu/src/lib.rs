#![no_std]
extern crate alloc;

mod api;
mod filters;
mod helpers;
mod home;
mod models;
mod reader;
mod settings;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, ImageResponse,
	Listing, ListingProvider, Manga, MangaPageResult, Page, PageContext, PageImageProcessor,
	Result, Source,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::{canvas::ImageRef, net::Request},
	prelude::*,
};
use api::{BASE_URL, BrowseParams, SORTS};
use models::{Series, SeriesDetail, chapter_list};

struct Toonkotsu;

impl Source for Toonkotsu {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some((key, _)) = query
			.as_deref()
			.map(str::trim)
			.and_then(helpers::keys_from_url)
		{
			let detail: SeriesDetail = api::parse(api::series_request(&key)?.send()?)?;
			let entry = detail
				.series
				.into_manga()
				.ok_or_else(|| error!("Toonkotsu did not return this series"))?;
			return Ok(MangaPageResult {
				entries: vec![entry],
				has_next_page: false,
			});
		}
		let mut params = BrowseParams {
			query: query.unwrap_or_default(),
			..BrowseParams::new("relevance", page)
		};

		for filter in filters {
			match filter {
				FilterValue::Sort {
					id,
					index,
					ascending,
				} if id == "sort" => {
					if let Some(sort) = SORTS.get(index as usize) {
						params.sort = (*sort).into();
						// Every other sort reads best first by default, so Title starts at A.
						let ascending = if *sort == "title" {
							!ascending
						} else {
							ascending
						};
						params.order = if ascending { "asc" } else { "desc" }.into();
					}
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"genre_mode" => params.genre_mode = value,
					"exclude_mode" => params.exclude_mode = value,
					_ => {}
				},
				FilterValue::Text { id, value } if id == "author" => params.author = value,
				FilterValue::Range { id, from, to } => {
					let bound = |value: Option<f32>| {
						value
							.map(|value| (value as i32).to_string())
							.unwrap_or_default()
					};
					match id.as_str() {
						"year" => (params.year_min, params.year_max) = (bound(from), bound(to)),
						"chapters" => {
							(params.chapters_min, params.chapters_max) = (bound(from), bound(to));
						}
						_ => {}
					}
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"demographics" => params.demographics = included,
					"statuses" => params.statuses = included,
					"languages" => params.original_languages = included,
					// Every tag group filters the one pair of catalogue parameters.
					_ if filters::GROUPS.iter().any(|(group, _)| *group == id) => {
						params.included_genres.extend(included);
						params.excluded_genres.extend(excluded);
					}
					_ => {}
				},
				_ => {}
			}
		}

		// A hidden tag the search asks for is shown for that search.
		let included = &params.included_genres;
		params.excluded_genres.retain(|tag| !included.contains(tag));

		// Best Match ranks by the search term, so the site's own order stands in without one.
		if params.sort == "relevance" && params.query.is_empty() {
			params.sort = "latest".into();
		}

		api::series_page(api::browse_request(&params)?.send()?, Series::into_manga)
	}

	/// One request carries both the details and the chapter list.
	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let SeriesDetail { series, chapters } =
			api::parse(api::series_request(&manga.key)?.send()?)?;
		if needs_chapters {
			manga.chapters = Some(chapter_list(
				chapters.unwrap_or_default(),
				settings::deduplicate_chapters(),
				settings::show_upload_size(),
			));
		}
		if needs_details {
			manga.copy_from(
				series
					.into_full_manga()
					.ok_or_else(|| error!("Toonkotsu did not return this series"))?,
			);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		reader::pages(&chapter.key)
	}
}

impl ListingProvider for Toonkotsu {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let response = home::section_request(&listing.id, page)?.send()?;
		api::series_page(response, Series::into_manga)
	}
}

impl ImageRequestProvider for Toonkotsu {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl PageImageProcessor for Toonkotsu {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		let (Some(key), Some(nonce)) = (context.get("key"), context.get("nonce")) else {
			return Ok(response.image);
		};
		let mut data = response.image.data();
		reader::decrypt_page(&mut data, key, nonce)?;
		Ok(ImageRef::new(&data))
	}
}

impl DeepLinkHandler for Toonkotsu {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(
			helpers::keys_from_url(&url).map(|(manga_key, chapter)| match chapter {
				Some(key) => DeepLinkResult::Chapter { manga_key, key },
				None => DeepLinkResult::Manga { key: manga_key },
			}),
		)
	}
}

register_source!(
	Toonkotsu,
	Home,
	ListingProvider,
	DynamicFilters,
	DynamicSettings,
	ImageRequestProvider,
	PageImageProcessor,
	DeepLinkHandler
);
