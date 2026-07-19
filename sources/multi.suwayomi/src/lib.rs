#![no_std]
extern crate alloc;

mod graphql;
mod models;
mod settings;

const CATEGORY_FILTER_ID: &str = "CATEGORY";

use crate::models::{
	FetchChapterPagesResponse, GraphQLResponse, MangaOnlyDescriptionResponse, MultipleCategories,
	MultipleChapters, MultipleMangas,
};
use aidoku::imports::std::{current_date, send_partial_result};
use aidoku::{
	AidokuError, BaseUrlProvider, BasicLoginHandler, Chapter, DynamicListings, FilterValue,
	ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext,
	Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use alloc::string::ToString;
use alloc::vec;
use base64::{Engine, engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}};

struct Suwayomi;

impl Suwayomi {
	fn send_graphql_token_request(
		&self,
		base_url: &str,
		token: Option<String>,
		body: String,
	) -> core::result::Result<Request, aidoku::imports::net::RequestError> {
		let mut req = Request::post(format!("{base_url}/api/graphql"))?
			.header("Content-Type", "application/json");
		if let Some(t) = token {
			req = req.header("Authorization", &format!("Bearer {t}"));
		}
		let req = req.body(body);
		Ok(req)
	}

	fn send_graphql_post_request(
		&self,
		base_url: &str,
		body: String,
	) -> core::result::Result<Request, aidoku::imports::net::RequestError> {
		self.send_graphql_token_request(base_url, None, body)
	}

	fn send_basic_auth_request(
		&self,
		base_url: &str,
		user: &str,
		pass: &str,
		body: String,
	) -> core::result::Result<Request, aidoku::imports::net::RequestError> {
		let auth = STANDARD.encode(format!("{user}:{pass}"));
		let req = self
			.send_graphql_post_request(base_url, body)?
			.header("Authorization", &format!("Basic {auth}"));
		Ok(req)
	}

	fn send_form_login_request(
		&self,
		base_url: &str,
		user: &str,
		pass: &str,
	) -> core::result::Result<Request, aidoku::imports::net::RequestError> {
		let form = format!(
			"user={}&pass={}",
			aidoku::helpers::uri::encode_uri_component(user),
			aidoku::helpers::uri::encode_uri_component(pass)
		);
		let req = Request::post(format!("{base_url}/login.html"))?
			.header("Content-Type", "application/x-www-form-urlencoded")
			.body(form);
		Ok(req)
	}

	fn perform_ui_login(&self, base_url: &str, user: &str, pass: &str) -> Result<()> {
		let mutation = serde_json::json!({
			"query": "mutation Login($input: LoginInput!) { login(input: $input) { accessToken refreshToken } }",
			"variables": {
				"input": {
					"username": user,
					"password": pass
				}
			}
		});

		let resp = Request::post(format!("{base_url}/api/graphql"))?
			.header("Content-Type", "application/json")
			.body(mutation.to_string())
			.json_owned::<GraphQLResponse<serde_json::Value>>()?;

		let login_payload = resp.data.get("login").ok_or_else(|| aidoku::error!("Missing login payload"))?;
		let access_token = login_payload.get("accessToken").and_then(|v| v.as_str()).ok_or_else(|| aidoku::error!("Missing accessToken"))?;
		let refresh_token = login_payload.get("refreshToken").and_then(|v| v.as_str()).ok_or_else(|| aidoku::error!("Missing refreshToken"))?;

		settings::set_tokens(access_token, refresh_token);
		Ok(())
	}

	fn perform_token_refresh(&self, base_url: &str, refresh_token: &str) -> Result<String> {
		let mutation = serde_json::json!({
			"query": "mutation Refresh($input: RefreshTokenInput!) { refreshToken(input: $input) { accessToken } }",
			"variables": {
				"input": {
					"refreshToken": refresh_token
				}
			}
		});

		let resp = Request::post(format!("{base_url}/api/graphql"))?
			.header("Content-Type", "application/json")
			.body(mutation.to_string())
			.json_owned::<GraphQLResponse<serde_json::Value>>()?;

		let payload = resp.data.get("refreshToken").ok_or_else(|| aidoku::error!("Missing refreshToken payload"))?;
		let access_token = payload.get("accessToken").and_then(|v| v.as_str()).ok_or_else(|| aidoku::error!("Missing accessToken"))?;

		settings::set_access_token(access_token);
		Ok(access_token.into())
	}

	fn get_valid_access_token(&self, base_url: &str) -> Result<String> {
		if settings::get_credentials().is_none() {
			settings::clear_tokens();
			return Err(aidoku::error!("Not authenticated"));
		}

		if let Some(token) = settings::get_access_token() {
			if !is_token_expired(&token) {
				return Ok(token);
			}
		}

		if let Some(refresh_token) = settings::get_refresh_token() {
			if let Ok(new_token) = self.perform_token_refresh(base_url, &refresh_token) {
				return Ok(new_token);
			}
		}

		if let Some((user, pass)) = settings::get_credentials() {
			self.perform_ui_login(base_url, &user, &pass)?;
			if let Some(new_token) = settings::get_access_token() {
				return Ok(new_token);
			}
		}

		Err(aidoku::error!("Not authenticated"))
	}

	fn graphql_request<T>(&self, body: serde_json::Value) -> Result<GraphQLResponse<T>>
	where
		T: serde::de::DeserializeOwned,
	{
		let base_url = settings::get_base_url()?;
		let auth_mode = settings::get_auth_mode();
		let body_str = body.to_string();

		let send_req = |with_basic: bool, token: Option<String>| -> Result<GraphQLResponse<T>> {
			let request = if let Some(t) = token {
				self.send_graphql_token_request(&base_url, Some(t), body_str.clone())?
			} else if with_basic && let Some((user, pass)) = settings::get_credentials() {
				self.send_basic_auth_request(&base_url, &user, &pass, body_str.clone())?
			} else {
				self.send_graphql_post_request(&base_url, body_str.clone())?
			};
			request.json_owned::<GraphQLResponse<T>>()
		};

		let do_login_html = || -> Result<()> {
			if let Some((user, pass)) = settings::get_credentials() {
				let _ = self
					.send_form_login_request(&base_url, &user, &pass)?
					.send()
					.ok();
			}
			Ok(())
		};

		match auth_mode.as_str() {
			"none" => send_req(false, None),
			"basic_auth" => send_req(true, None),
			"simple_login" => {
				do_login_html()?;
				send_req(false, None)
			}
			"ui_login" => {
				let token = self.get_valid_access_token(&base_url)?;
				let resp = send_req(false, Some(token));
				if resp.is_err() {
					settings::clear_tokens();
					if let Ok(new_token) = self.get_valid_access_token(&base_url) {
						send_req(false, Some(new_token))
					} else {
						resp
					}
				} else {
					resp
				}
			}
			_ => {
				// auto:
				if let Ok(token) = self.get_valid_access_token(&base_url) {
					let mut resp = send_req(false, Some(token));
					if resp.is_ok() {
						return resp;
					}
					settings::clear_tokens();
					if let Ok(new_token) = self.get_valid_access_token(&base_url) {
						resp = send_req(false, Some(new_token));
						if resp.is_ok() {
							return resp;
						}
					}
				}
				let resp = send_req(true, None);
				if resp.is_err() {
					do_login_html()?;
					return send_req(true, None);
				}
				resp
			}
		}
	}

	fn execute_query<T>(
		&self,
		gql: graphql::GraphQLQuery,
		variables: Option<serde_json::Value>,
	) -> Result<GraphQLResponse<T>>
	where
		T: serde::de::DeserializeOwned,
	{
		let mut body = serde_json::json!({
			"operationName": gql.operation_name,
			"query": gql.query,
		});

		if let Some(vars) = variables {
			body["variables"] = vars;
		}

		self.graphql_request(body)
	}
}

fn get_jwt_exp(token: &str) -> Option<u64> {
	let mut parts = token.split('.');
	let _header = parts.next()?;
	let payload_b64 = parts.next()?;

	let decoded = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
	let decoded_str = core::str::from_utf8(&decoded).ok()?;
	let json: serde_json::Value = serde_json::from_str(decoded_str).ok()?;
	let exp = json.get("exp")?.as_u64()?;
	Some(exp)
}

fn is_token_expired(token: &str) -> bool {
	if let Some(exp) = get_jwt_exp(token) {
		let now = current_date() as u64;
		now + 10 >= exp
	} else {
		true
	}
}

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
				FilterValue::Check { id, value } => {
					if id == CATEGORY_FILTER_ID {
						// This is special cased since the "Default" category means you don't have
						// any categories attached to the manga.
						let filter_value = if value == 0 {
							serde_json::json!({"isNull": true})
						} else {
							serde_json::json!({"equalTo": value})
						};
						manga_filter.insert("categoryId".to_string(), filter_value);
					}
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

		let response = self.execute_query::<MultipleMangas>(
			graphql::GraphQLQuery::SEARCH_MANGA_LIST,
			Some(json_value),
		)?;

		let base_url = settings::get_base_url()?;
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
		let manga_id = manga
			.key
			.parse::<i32>()
			.map_err(|_| AidokuError::DeserializeError)?;
		if needs_details {
			let response = self.execute_query::<MangaOnlyDescriptionResponse>(
				graphql::GraphQLQuery::MANGA_DESCRIPTION,
				Some(serde_json::json!({
					"mangaId": manga_id
				})),
			)?;

			manga.description = Some(response.data.manga.description);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			let response = self.execute_query::<MultipleChapters>(
				graphql::GraphQLQuery::MANGA_CHAPTERS,
				Some(serde_json::json!({
					"mangaId": manga_id
				})),
			)?;

			let base_url = settings::get_base_url()?;
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
		let chapter_id = chapter
			.key
			.parse::<i32>()
			.map_err(|_| AidokuError::DeserializeError)?;

		let response = self.execute_query::<FetchChapterPagesResponse>(
			graphql::GraphQLQuery::CHAPTER_PAGES,
			Some(serde_json::json!({
				"input": {
					"chapterId": chapter_id
				}
			})),
		)?;

		let base_url = settings::get_base_url()?;
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
		let category_id = listing
			.id
			.parse::<i32>()
			.map_err(|_| AidokuError::DeserializeError)?;

		self.get_search_manga_list(
			None,
			page,
			vec![
				FilterValue::Sort {
					id: String::default(),
					index: 0,
					ascending: true,
				},
				FilterValue::Check {
					id: CATEGORY_FILTER_ID.to_string(),
					value: category_id,
				},
			],
		)
	}
}

impl DynamicListings for Suwayomi {
	fn get_dynamic_listings(&self) -> Result<Vec<Listing>> {
		let response =
			self.execute_query::<MultipleCategories>(graphql::GraphQLQuery::CATEGORIES, None)?;

		let categories = response.data.categories.nodes;
		let total_count = categories.len();

		Ok(categories
			.into_iter()
			.map(|c| c.into_listing(total_count))
			.collect())
	}
}

impl BaseUrlProvider for Suwayomi {
	fn get_base_url(&self) -> Result<String> {
		settings::get_base_url()
	}
}

impl BasicLoginHandler for Suwayomi {
	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		let base_url = settings::get_base_url()?;
		let auth_mode = settings::get_auth_mode();

		let send_basic_req = || {
			let body = serde_json::json!({
				"operationName": graphql::GraphQLQuery::CATEGORIES.operation_name,
				"query": graphql::GraphQLQuery::CATEGORIES.query,
			});
			self.send_basic_auth_request(&base_url, &username, &password, body.to_string())?
				.send()
		};

		let send_form_req = || {
			self.send_form_login_request(&base_url, &username, &password)?
				.send()
		};

		let send_ui_login = || {
			self.perform_ui_login(&base_url, &username, &password)
		};

		match auth_mode.as_str() {
			"none" => Ok(true),
			"basic_auth" => {
				let resp = send_basic_req()?;
				Ok(resp.status_code() == 200)
			}
			"simple_login" => {
				let resp = send_form_req()?;
				Ok(resp.status_code() == 200)
			}
			"ui_login" => {
				Ok(send_ui_login().is_ok())
			}
			_ => {
				// auto: try basic auth first
				if let Ok(resp) = send_basic_req()
					&& resp.status_code() == 200
				{
					return Ok(true);
				}
				// try form login next
				if let Ok(resp) = send_form_req()
					&& resp.status_code() == 200
				{
					return Ok(true);
				}
				// try ui login next
				if let Ok(_) = send_ui_login() {
					return Ok(true);
				}
				Ok(false)
			}
		}
	}
}

impl ImageRequestProvider for Suwayomi {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let base_url = settings::get_base_url()?;
		let auth_mode = settings::get_auth_mode();

		let mut req = Request::get(url)?;

		match auth_mode.as_str() {
			"basic_auth" => {
				if let Some((user, pass)) = settings::get_credentials() {
					let auth = STANDARD.encode(format!("{user}:{pass}"));
					req = req.header("Authorization", &format!("Basic {auth}"));
				}
			}
			"ui_login" => {
				if let Ok(token) = self.get_valid_access_token(&base_url) {
					req = req.header("Authorization", &format!("Bearer {token}"));
				}
			}
			"none" | "simple_login" => {}
			_ => {
				// auto:
				if let Ok(token) = self.get_valid_access_token(&base_url) {
					req = req.header("Authorization", &format!("Bearer {token}"));
				} else if let Some((user, pass)) = settings::get_credentials() {
					let auth = STANDARD.encode(format!("{user}:{pass}"));
					req = req.header("Authorization", &format!("Basic {auth}"));
				}
			}
		}

		Ok(req)
	}
}

register_source!(
	Suwayomi,
	ListingProvider,
	BaseUrlProvider,
	DynamicListings,
	BasicLoginHandler,
	ImageRequestProvider
);

#[cfg(test)]
mod tests {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn current_date() -> f64 {
		// Mock current time: 1782200000
		1782200000.0
	}

	#[test]
	fn test_jwt_expiry() {
		// {"exp":1782297600} -> eyJleHAiOjE3ODIyOTc2MDB9
		let mock_token = "header.eyJleHAiOjE3ODIyOTc2MDB9.signature";
		assert_eq!(get_jwt_exp(mock_token), Some(1782297600));

		// Test is_token_expired
		// 1782297600 is in the far future relative to 1782200000, so it shouldn't be expired
		assert!(!is_token_expired(mock_token));

		// Test expired token (exp: 1000)
		// {"exp":1000} -> eyJleHAiOjEwMDB9
		let mock_expired_token = "header.eyJleHAiOjEwMDB9.signature";
		assert_eq!(get_jwt_exp(mock_expired_token), Some(1000));
		assert!(is_token_expired(mock_expired_token));
	}
}
