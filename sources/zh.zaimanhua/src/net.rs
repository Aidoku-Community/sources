use crate::models;
use crate::settings;

use crate::{V4_API_URL, USER_AGENT};
use aidoku::{
	Result,
	alloc::{String, Vec, format, string::ToString},
	imports::net::Request,
	serde::de::DeserializeOwned,
};

pub const ACCOUNT_API: &str = "https://account-api.zaimanhua.com/v1/";
pub const SIGN_API: &str = "https://i.zaimanhua.com/lpi/v1/";
pub const NEWS_URL: &str = "https://news.zaimanhua.com";

// === HTTP Request Helpers ===

pub fn md5_hex(input: &str) -> String {
	let digest = md5::compute(input.as_bytes());
	format!("{:x}", digest)
}

pub fn get_request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?.header("User-Agent", USER_AGENT))
}

pub fn post_request(url: &str) -> Result<Request> {
	Ok(Request::post(url)?
		.header("User-Agent", USER_AGENT)
		.header("Content-Type", "application/x-www-form-urlencoded"))
}

pub fn auth_request(url: &str, token: Option<&str>) -> Result<Request> {
	match token {
		Some(t) => Ok(Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header("Authorization", &format!("Bearer {}", t))),
		None => get_request(url),
	}
}

// === API Methods ===

/// Attempts to refresh the token using stored credentials.
/// Returns Ok(Some(new_token)) if successful, Ok(None) if no credentials or login failed.
pub fn try_refresh_token() -> Result<Option<String>> {
	if let Some((username, password)) = settings::get_credentials()
		&& let Ok(Some(new_token)) = login(&username, &password)
	{
		settings::set_token(&new_token);
		return Ok(Some(new_token));
	}
	Ok(None)
}

pub fn send_authed_request<T: DeserializeOwned>(
	url: &str,
	token: Option<&str>,
) -> Result<models::ApiResponse<T>> {
	let req = auth_request(url, token)?;
	let resp: models::ApiResponse<T> = req.send()?.get_json_owned()?;

	if resp.errno.unwrap_or(0) == 99
		&& let Ok(Some(new_token)) = try_refresh_token() 
	{
		// Retry with new token
		return auth_request(url, Some(&new_token))?.send()?.get_json_owned();
	}
	Ok(resp)
}

/// Authenticates via username/password and extracts the user token.
pub fn login(username: &str, password: &str) -> Result<Option<String>> {
	let password_hash = md5_hex(password);
	let url = format!("{}login/passwd", ACCOUNT_API);
	let body = format!("username={}&passwd={}", username, password_hash);

	let response: models::ApiResponse<models::LoginData> =
		post_request(&url)?.body(body.as_bytes()).json_owned()?;

	if response.errno.unwrap_or(-1) != 0 {
		return Ok(None);
	}

	Ok(response.data.and_then(|d| d.user).and_then(|u| u.token))
}

/// Perform daily check-in (POST request required!)
pub fn check_in(token: &str) -> Result<bool> {
	let url = format!("{}task/sign_in", SIGN_API);

	let response: models::ApiResponse<aidoku::serde::de::IgnoredAny> = Request::post(&url)?
		.header("User-Agent", USER_AGENT)
		.header("Authorization", &format!("Bearer {}", token))
		.json_owned()?;

	Ok(response.errno.unwrap_or(-1) == 0)
}

/// Get user info (for level, points, VIP status etc)
pub fn get_user_info(token: &str) -> Result<models::UserInfoData> {
	let url = format!("{}userInfo/get", SIGN_API);
	let response: models::ApiResponse<models::UserInfoData> =
		send_authed_request(&url, Some(token))?;
	response
		.data
		.ok_or_else(|| aidoku::error!("Missing user info"))
}

// === Hidden Content Scanner ===

/// Scanner for hidden content, implementing Iterator for lazy fetching
pub struct HiddenContentScanner {
	current_page: i32,
	scanned_batches: i32,
	max_batches: i32,
	token: Option<String>,
}

impl HiddenContentScanner {
	pub fn new(start_page: i32, max_batches: i32, token: Option<&str>) -> Self {
		Self {
			current_page: start_page,
			scanned_batches: 0,
			max_batches,
			token: token.map(|s| s.to_string()),
		}
	}
}

impl Iterator for HiddenContentScanner {
	type Item = Vec<models::FilterItem>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.scanned_batches >= self.max_batches {
			return None;
		}

		let mut batch_found = false;
		let mut items: Vec<models::FilterItem> = Vec::new();

		while self.scanned_batches < self.max_batches {
			self.scanned_batches += 1;
			let end_page = self.current_page + 4;

			// === Parallel Batch Scan ===
			// Efficiently fetch multiple pages at once to find hidden content quickly.
			let make_requests = |token: Option<&str>| -> Vec<Request> {
				(self.current_page..=end_page)
					.filter_map(|p| {
						let url = format!("{}/comic/filter/list?sortType=1&page={}&size=100", V4_API_URL, p);
						auth_request(&url, token).ok()
					})
					.collect()
			};

			let requests = make_requests(self.token.as_deref());
			let responses = Request::send_all(requests);

			let mut parsed_responses: Vec<models::ApiResponse<models::FilterData>> = responses
				.into_iter()
				.flatten()
				.filter_map(|resp| resp.get_json_owned().ok())
				.collect();

			let has_auth_error = parsed_responses.iter().any(|r| r.errno.unwrap_or(0) == 99);

			if has_auth_error
				&& let Ok(Some(new_token)) = try_refresh_token()
			{
				self.token = Some(new_token.clone());
				
				// Retry the batch with the new token
				let requests = make_requests(Some(&new_token));
				let responses = Request::send_all(requests);
				parsed_responses = responses
					.into_iter()
					.flatten()
					.filter_map(|resp| resp.get_json_owned().ok())
					.collect();
			}

			items = parsed_responses
				.into_iter()
				.filter_map(|r| r.data)
				.flat_map(|data| data.comic_list)
				.collect();

			self.current_page += 5;

			if !items.is_empty() {
				batch_found = true;
				break;
			}
			// Early exit: if first batch is empty, don't waste time
			if self.scanned_batches == 1 {
				break;
			}
		}

		if batch_found {
			Some(items)
		} else {
			None
		}
	}
}
