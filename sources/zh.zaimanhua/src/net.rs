use crate::models;
use crate::settings;

use crate::{ACCOUNT_API, SIGN_API, V4_API_URL, USER_AGENT};
use aidoku::{
	Result,
	alloc::{String, Vec, format, string::ToString},
	imports::net::{Request, Response},
	serde::de::DeserializeOwned,
};

use aidoku::helpers::uri::encode_uri_component;

/// Resolves URL through proxy if proxy toggle is enabled and valid URL is configured.
pub fn resolve_url(url: &str) -> String {
	if settings::get_use_proxy()
		&& let Some(proxy) = settings::get_proxy_url()
	{
		let encoded = encode_uri_component(url);
		return format!("{}/?url={}", proxy, encoded);
	}
	url.into()
}

// === Proxy Block List ===

/// URLs that should be blocked in proxy mode (requests not sent).
/// Uses existing constants for single source of truth.
const PROXY_BLOCKED_URLS: &[&str] = &[
//	NEWS_URL,                       // Banner
//	SIGN_API,                       // User Info API
//	V4_API_URL,                     // Main API (Content, Listings)
//	WEB_URL,                        // Web View
//	ACCOUNT_API,                    // Login API
];

/// Check if a URL should be blocked in proxy mode.
pub fn should_block(url: &str) -> bool {
	settings::get_use_proxy() &&
		PROXY_BLOCKED_URLS.iter().any(|blocked| url.starts_with(blocked))
}


// === HTTP Request Helpers ===

pub fn md5_hex(input: &str) -> String {
	let digest = md5::compute(input.as_bytes());
	format!("{:x}", digest)
}

pub fn get_request(url: &str) -> Result<Request> {
	let resolved = resolve_url(url);
	Ok(Request::get(&resolved)?.header("User-Agent", USER_AGENT))
}

pub fn post_request(url: &str) -> Result<Request> {
	let resolved = resolve_url(url);
	Ok(Request::post(&resolved)?
		.header("User-Agent", USER_AGENT)
		.header("Content-Type", "application/x-www-form-urlencoded"))
}

pub fn auth_request(url: &str, token: Option<&str>) -> Result<Request> {
	let resolved = resolve_url(url);
	match token {
		Some(t) => Ok(Request::get(&resolved)?
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

/// Helper to fetch and cache user profile (Level & Sign status)
/// Returns Ok(()) on success, Err if network fails.
pub fn refresh_user_profile(token: &str) -> Result<()> {
	let info_data = get_user_info(token)?;
	if let Some(info) = info_data.user_info {
		let level = info.level.unwrap_or(0) as i32;
		let is_sign = info.is_sign.unwrap_or(false);
		settings::set_user_cache(level, is_sign);
	}
	Ok(())
}

/// Perform silent background updates: auto check-in and cache refresh.
/// Called from home page to avoid blocking user. Errors are swallowed.
pub fn perform_silent_updates(token: &str) {
	let mut checkin_performed = false;

	// Auto Check-in
	if settings::get_auto_checkin()
		&& !settings::has_checkin_flag()
		&& !should_block(crate::SIGN_API)
		&& check_in(token).ok() == Some(true)
	{
		settings::set_last_checkin();
		checkin_performed = true;
		let _ = refresh_user_profile(token);
	}

	// Stale Cache Update (if no check-in just happened)
	if !checkin_performed && settings::is_cache_stale() && !should_block(crate::SIGN_API) {
		let _ = refresh_user_profile(token);
	}
}

// === Request Batch Builder ===

pub struct RequestBatch {
	requests: Vec<Request>,
	index_map: Vec<usize>,
	total_slots: usize,
}

impl RequestBatch {
	pub fn new() -> Self {
		Self { requests: Vec::new(), index_map: Vec::new(), total_slots: 0 }
	}

	/// Add a GET request. automatically checking for blocking.
	pub fn get(&mut self, url: &str) -> Result<usize> {
		if should_block(url) {
			return Ok(self.add_if(None));
		}
		Ok(self.add(get_request(url)?))
	}

	/// Add an Authenticated GET request, automatically checking for blocking.
	pub fn auth(&mut self, url: &str, token: Option<&str>) -> Result<usize> {
		if should_block(url) {
			return Ok(self.add_if(None));
		}
		Ok(self.add(auth_request(url, token)?))
	}

	/// Add a request that MUST execute. Returns the slot index.
	fn add(&mut self, req: Request) -> usize {
		self.add_if(Some(req))
	}

	/// Conditionally add a request. If None, it takes a slot but executes nothing.
	pub fn add_if(&mut self, req: Option<Request>) -> usize {
		let slot = self.total_slots;
		if let Some(r) = req {
			self.requests.push(r);
			self.index_map.push(slot);
		}
		self.total_slots += 1;
		slot
	}

	/// If blocked, zero network overhead is incurred.
	pub fn add_unless_blocked(&mut self, url: &str) -> usize {
		let req = if should_block(url) {
			None
		} else {
			get_request(url).ok()
		};
		self.add_if(req)
	}

	/// Execute all accumulated requests and map responses back to their slots.
	pub fn send_all(self) -> Vec<Option<Response>> {
		let responses = Request::send_all(self.requests);
		let mut result: Vec<Option<Response>> = Vec::with_capacity(self.total_slots);
		for _ in 0..self.total_slots {
			result.push(None);
		}
		for (resp, slot) in responses.into_iter().zip(self.index_map) {
			if let Ok(r) = resp {
				result[slot] = Some(r);
			}
		}
		result
	}
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
				// Retry the batch with the new token
				let requests = make_requests(Some(&new_token));
				let responses = Request::send_all(requests);
				parsed_responses = responses
					.into_iter()
					.flatten()
					.filter_map(|resp| resp.get_json_owned().ok())
					.collect();

				self.token = Some(new_token);
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
