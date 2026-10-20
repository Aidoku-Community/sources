use crate::{Params, models::*};
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, Manga, MangaPageResult, MultiSelectFilter, Page,
	PageContent, Result, SelectFilter, SortFilter, SortFilterDefault,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		net::Request,
		std::{current_date, send_partial_result},
	},
	prelude::*,
};
use serde::de::DeserializeOwned;

const STATUSES: [(&str, &str); 6] = [
	("All", "All"),
	("Ongoing", "Ongoing"),
	("Hiatus", "Hiatus"),
	("Dropped", "Dropped"),
	("Completed", "Completed"),
	("Cancelled", "Canceled"),
];

const SORTS: [(&str, &str); 4] = [
	("Title", "title"),
	("Views", "total_views"),
	("Latest", "latest"),
	("Created at", "created_at"),
];

const DEFAULT_SORT: i32 = 3;

fn api_get<T: DeserializeOwned>(params: &Params, path: &str) -> Result<T> {
	Request::get(format!("{}{path}", params.api_url))?
		.header("Referer", &format!("{}/", params.base_url))
		.json_owned()
}

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		mut filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if !filters
			.iter()
			.any(|f| matches!(f, FilterValue::Sort { .. }))
		{
			filters.push(FilterValue::Sort {
				id: "orderBy".into(),
				index: DEFAULT_SORT,
				ascending: false,
			});
		}
		let mut qs = QueryParameters::new();
		if let Some(query) = &query {
			qs.push("query_string", Some(query.trim()));
		}
		qs.push("series_type", Some("Comic"));
		qs.push("adult", Some("true"));
		qs.push("page", Some(&page.to_string()));
		qs.push("perPage", Some("12"));
		for filter in filters {
			match filter {
				FilterValue::Select { id, value } if id == "genre" => {
					let tags: Vec<Tag> = api_get(params, "/tags")?;
					let Some(tag) = tags.into_iter().find(|t| t.name == value) else {
						return Ok(MangaPageResult::default());
					};
					qs.push("tags_ids", Some(&format!("[{}]", tag.id)));
				}
				FilterValue::Select { id, value } => qs.push(&id, Some(&value)),
				FilterValue::Sort {
					id,
					index,
					ascending,
				} => {
					let sort = SORTS.get(index as usize).map(|s| s.1);
					let ascending =
						ascending ^ (params.latest_sort_ascending && sort == Some("latest"));
					qs.push(&id, sort);
					qs.push("order", Some(if ascending { "asc" } else { "desc" }));
				}
				FilterValue::MultiSelect { id, included, .. } => {
					qs.push(&id, Some(&format!("[{}]", included.join(","))))
				}
				_ => {}
			}
		}

		let data: Paginated<Series> = api_get(params, &format!("/query?{qs}"))?;

		Ok(MangaPageResult {
			has_next_page: data.meta.has_next_page(),
			entries: data
				.data
				.into_iter()
				.map(Series::into_basic_manga)
				.collect(),
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let (slug, cached_id) = split_key(&manga.key);
		let slug = slug.to_string();

		let series_id = match cached_id {
			Some(id) if !needs_details => id,
			_ => {
				let series: Series = api_get(params, &format!("/series/{slug}"))?;
				let id = series.id;
				if needs_details {
					manga.copy_from(series.into_manga(params));
					if needs_chapters {
						send_partial_result(&manga);
					}
				}
				id
			}
		};

		if needs_chapters {
			let now = current_date();
			let mut chapters = Vec::new();
			let mut page = 1;
			loop {
				let data: Paginated<HeanChapter> = api_get(
					params,
					&format!("/chapter/query?page={page}&perPage=1000&series_id={series_id}"),
				)?;
				chapters.extend(
					data.data
						.into_iter()
						.map(|c| c.into_chapter(&params.base_url, &slug))
						.filter(|c| c.date_uploaded.is_none_or(|d| d <= now)),
				);
				if !data.meta.has_next_page() {
					break;
				}
				page += 1;
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let data: ChapterResponse = api_get(
			params,
			&format!("/chapter/{}/{}", split_key(&manga.key).0, chapter.key),
		)?;

		Ok(data
			.chapter
			.chapter_data
			.images
			.unwrap_or_default()
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}

	fn get_dynamic_filters(&self, params: &Params) -> Result<Vec<Filter>> {
		let tags: Vec<Tag> = api_get(params, "/tags")?;

		let mut filters = vec![
			SortFilter {
				id: "orderBy".into(),
				title: Some("Sort By".into()),
				options: SORTS.iter().map(|s| s.0.into()).collect(),
				default: Some(SortFilterDefault {
					index: DEFAULT_SORT,
					ascending: false,
				}),
				..Default::default()
			}
			.into(),
			SelectFilter {
				id: "status".into(),
				title: Some("Status".into()),
				options: STATUSES.iter().map(|s| s.0.into()).collect(),
				ids: Some(STATUSES.iter().map(|s| s.1.into()).collect()),
				..Default::default()
			}
			.into(),
		];

		if !tags.is_empty() {
			let (options, ids) = tags
				.into_iter()
				.map(|t| (t.name.into(), t.id.to_string().into()))
				.unzip();
			filters.push(
				MultiSelectFilter {
					id: "tags_ids".into(),
					title: Some("Genres".into()),
					is_genre: true,
					uses_tag_style: true,
					options,
					ids: Some(ids),
					..Default::default()
				}
				.into(),
			);
		}

		Ok(filters)
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.strip_prefix(params.base_url.as_ref())
			.and_then(|p| p.strip_prefix("/series/"))
			.and_then(|p| p.split(['?', '#']).next())
			.map(|p| p.trim_end_matches('/'))
		else {
			return Ok(None);
		};

		let (slug, chapter_key) = match path.split_once('/') {
			Some((slug, key)) => (slug, Some(key)),
			None => (path, None),
		};
		let manga_key = api_get::<Series>(params, &format!("/series/{slug}"))?.key();

		Ok(Some(match chapter_key {
			Some(key) => DeepLinkResult::Chapter {
				manga_key,
				key: key.into(),
			},
			None => DeepLinkResult::Manga { key: manga_key },
		}))
	}
}
