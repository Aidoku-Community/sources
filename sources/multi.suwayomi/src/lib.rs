#![no_std]
extern crate alloc;

mod graphql;
mod models;
mod settings;

use crate::models::{
	FetchChapterPagesResponse, GraphQLResponse, MangaOnlyDescriptionResponse, MultipleChapters,
	MultipleMangas,
};
use aidoku::imports::std::send_partial_result;
use aidoku::{
	AidokuError, Chapter, FilterValue, Listing, ListingProvider, Manga, MangaPageResult, Page,
	PageContent, Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use alloc::string::ToString;
use alloc::vec;

struct Suwayomi;

impl Source for Suwayomi {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut condition = serde_json::Map::new();
		condition.insert("inLibrary".to_string(), serde_json::json!(true));

		let mut order: Vec<serde_json::Value> = Vec::new();
		let mut manga_filter = serde_json::Map::new();

		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					let property = match index {
						0 => "TITLE",
						1 => "IN_LIBRARY_AT",
						2 => "LAST_FETCHED_AT",
						_ => continue,
					};
					order.push(serde_json::json!({
						"by": property,
						"byType": if ascending { "ASC" } else { "DESC" }
					}));
				}
				_ => continue,
			}
		}

		if let Some(query) = query {
			manga_filter.insert(
				"title".to_string(),
				serde_json::json!({
					"likeInsensitive": format!("%{}%", query)
				}),
			);
		}

		let mut variables = serde_json::Map::new();
		variables.insert(
			"condition".to_string(),
			serde_json::Value::Object(condition),
		);
		variables.insert("order".to_string(), serde_json::Value::Array(order));
		variables.insert(
			"filter".to_string(),
			serde_json::Value::Object(manga_filter),
		);

		let json_value = serde_json::Value::Object(variables);

		let gql = graphql::GraphQLQuery::SEARCH_MANGA_LIST;
		let body = serde_json::json!({
			"operationName": gql.operation_name,
			"query": gql.query,
			"variables": json_value,
		});

		let base_url = settings::get_base_url();
		let data = Request::post(format!("{base_url}/api/graphql"))?
			.header("Content-Type", "application/json")
			.body(body.to_string())
			.data()?;

		let response = serde_json::from_slice::<GraphQLResponse<MultipleMangas>>(&data)
			.map_err(|_| AidokuError::JsonParseError)?;

		let base_url = settings::get_base_url();
		Ok(MangaPageResult {
			entries: response
				.data
				.mangas
				.nodes
				.into_iter()
				.map(|m| m.into_manga(&base_url))
				.collect(),
			has_next_page: false,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_id = manga.key.parse::<i32>().expect("Invalid number");
		let base_url = settings::get_base_url();
		if needs_details {
			let gql = graphql::GraphQLQuery::MANGA_DESCRIPTION;
			let variables = serde_json::json!({
				"mangaId": manga_id
			});

			let body = serde_json::json!({
				"operationName": gql.operation_name,
				"query": gql.query,
				"variables": variables,
			});

			let data = Request::post(format!("{base_url}/api/graphql"))?
				.header("Content-Type", "application/json")
				.body(body.to_string())
				.data()?;

			let response =
				serde_json::from_slice::<GraphQLResponse<MangaOnlyDescriptionResponse>>(&data)
					.map_err(|_| AidokuError::JsonParseError)?;

			manga.description = Some(response.data.manga.description);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			let gql = graphql::GraphQLQuery::MANGA_CHAPTERS;
			let variables = serde_json::json!({
				"mangaId": manga_id
			});

			let body = serde_json::json!({
				"operationName": gql.operation_name,
				"query": gql.query,
				"variables": variables,
			});

			let data = Request::post(format!("{base_url}/api/graphql"))?
				.header("Content-Type", "application/json")
				.body(body.to_string())
				.data()?;

			let response = serde_json::from_slice::<GraphQLResponse<MultipleChapters>>(&data)
				.map_err(|_| AidokuError::JsonParseError)?;

			manga.chapters = Some(
				response
					.data
					.chapters
					.nodes
					.into_iter()
					.map(|c| c.into_chapter(&base_url, manga_id))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_id = chapter.key.parse::<i32>().expect("Invalid chapter ID");

		let gql = graphql::GraphQLQuery::CHAPTER_PAGES;
		let variables = serde_json::json!({
			"input": {
				"chapterId": chapter_id
			}
		});

		let body = serde_json::json!({
			"operationName": gql.operation_name,
			"query": gql.query,
			"variables": variables,
		});

		let base_url = settings::get_base_url();
		let data = Request::post(format!("{base_url}/api/graphql"))?
			.header("Content-Type", "application/json")
			.body(body.to_string())
			.data()?;

		let response = serde_json::from_slice::<GraphQLResponse<FetchChapterPagesResponse>>(&data)
			.map_err(|_| AidokuError::JsonParseError)?;

		let base_url = settings::get_base_url();
		Ok(response
			.data
			.fetch_chapter_pages
			.pages
			.into_iter()
			.map(|url| {
				let full_url = format!("{}{}", base_url, url);
				Page {
					content: PageContent::Url(full_url, None),
					..Default::default()
				}
			})
			.collect())
	}
}

impl ListingProvider for Suwayomi {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"Library" => self.get_search_manga_list(
				None,
				page,
				vec![FilterValue::Sort {
					id: String::default(),
					index: 0,
					ascending: true,
				}],
			),
			_ => Err(AidokuError::Unimplemented),
		}
	}
}

register_source!(Suwayomi, ListingProvider);
