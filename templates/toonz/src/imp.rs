use crate::{
	Params,
	helpers::{
		SORTS, SearchParams, chapter_items, details, genre_options, narrowed, parse_series_path,
		reader_page, series_page, series_summary, slugify,
	},
	home,
	models::ReaderImage,
	rsc,
};
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, GroupSetting, HomeLayout, Listing, Manga,
	MangaPageResult, MultiSelectFilter, MultiSelectSetting, Page, Result, Setting,
	alloc::{String, Vec, vec},
	imports::std::send_partial_result,
	prelude::*,
};

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some((key, _)) = query
			.as_deref()
			.and_then(|url| url.trim().strip_prefix(params.base_url))
			.and_then(|path| parse_series_path(params, path))
			&& let Ok(document) = series_page(params, key)
			&& let Ok((title, cover)) = series_summary(&document)
		{
			return Ok(MangaPageResult {
				entries: vec![Manga {
					key: key.into(),
					title,
					cover,
					..Default::default()
				}],
				has_next_page: false,
			});
		}

		let mut search = SearchParams::new(params, query);
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(sort) = SORTS.get(index as usize) {
						search.sort = sort;
					}
				}
				FilterValue::Select { id, value } if !value.is_empty() => match id.as_str() {
					// A tapped tag arrives as its name; every genre's slug is its slugified name.
					"genre" => search.genres.push(slugify(&value)),
					"status" => search.status = Some(value),
					"rating" => search.content.rating = value,
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"genre" => {
						search.genres.extend(included);
						search.excluded_genres.extend(excluded);
					}
					"type" => search.content.types = narrowed(included, params.type_count),
					_ => {}
				},
				FilterValue::Text { id, value } if id == "author" => search.author = Some(value),
				_ => {}
			}
		}
		// A tag included in a search overrides its exclusion in settings.
		let genres = &search.genres;
		search.content.blocked.retain(|slug| !genres.contains(slug));

		search.page(params, page)
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let Some(sort) = SORTS.iter().find(|sort| **sort == listing.id) else {
			bail!("Unknown listing");
		};
		SearchParams {
			sort,
			..SearchParams::new(params, None)
		}
		.page(params, page)
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let page = series_page(params, &manga.key)?;
		if needs_details {
			details(&page, &mut manga)?;
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(
				chapter_items(params, &page)?
					.into_iter()
					.filter_map(|chapter| chapter.into_chapter(params, &manga.key))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let body = match reader_page(params, &manga.key, &chapter.key)? {
			Some(body) => body,
			// List-view chapter numbers can omit a slug suffix.
			None => {
				let Ok(number) = chapter.key.parse::<f32>() else {
					bail!("Chapter not found");
				};
				let page = series_page(params, &manga.key)?;
				let key = chapter_items(params, &page)?
					.into_iter()
					.find(|item| item.number() == Some(number))
					.map(|item| String::from(item.key()))
					.ok_or_else(|| error!("Chapter not found"))?;
				reader_page(params, &manga.key, &key)?.ok_or_else(|| error!("Chapter not found"))?
			}
		};
		self.parse_page_list(params, &body)
	}

	fn parse_page_list(&self, params: &Params, body: &str) -> Result<Vec<Page>> {
		rsc::reader_images::<ReaderImage>(body)?
			.into_iter()
			.map(|image| image.into_page(params.base_url))
			.collect()
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		home::get_home(params)
	}

	fn get_dynamic_filters(&self, params: &Params) -> Result<Vec<Filter>> {
		let (names, slugs) = genre_options(params)?;
		Ok(vec![
			MultiSelectFilter {
				id: "genre".into(),
				title: Some("Tags".into()),
				is_genre: true,
				uses_tag_style: true,
				can_exclude: true,
				options: names,
				ids: Some(slugs),
				..Default::default()
			}
			.into(),
		])
	}

	// Dynamic because each site has its own genre list, read from its API, and the site
	// hides excluded genres itself through the content cookie.
	fn get_dynamic_settings(&self, params: &Params) -> Result<Vec<Setting>> {
		let (names, slugs) = genre_options(params)?;
		Ok(vec![
			GroupSetting {
				title: "Excluded Genres".into(),
				footer: Some(
					"Titles with an excluded genre are hidden everywhere. An included tag \
					 overrides this for one search."
						.into(),
				),
				items: vec![
					MultiSelectSetting {
						key: "excludedGenres".into(),
						title: "Excluded Genres".into(),
						values: slugs,
						titles: Some(names),
						refreshes: Some(vec!["content".into(), "listings".into()]),
						..Default::default()
					}
					.into(),
				],
				..Default::default()
			}
			.into(),
		])
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(url
			.strip_prefix(params.base_url)
			.and_then(|path| parse_series_path(params, path))
			.map(|(key, chapter)| match chapter {
				Some(chapter) => DeepLinkResult::Chapter {
					manga_key: key.into(),
					key: chapter.into(),
				},
				None => DeepLinkResult::Manga { key: key.into() },
			}))
	}
}
