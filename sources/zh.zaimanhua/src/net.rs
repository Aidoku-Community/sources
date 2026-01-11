use crate::models;
use crate::settings;
use aidoku::{
	Result,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::encode_uri_component,
	imports::net::Request,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

pub const ACCOUNT_API: &str = "https://account-api.zaimanhua.com/v1/";
pub const SIGN_API: &str = "https://i.zaimanhua.com/lpi/v1/";
pub const V4_API_URL: &str = "https://v4api.zaimanhua.com/app/v1";
pub const NEWS_URL: &str = "https://news.zaimanhua.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 10) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

// === API URL Builder ===
// Centralized enum for type-safe URL construction.
/// Centralized URL construction for all API endpoints
pub enum Url<'a> {
	/// Search by keyword: /search/index?keyword={}&source=0&page={}&size={}
	Search { keyword: &'a str, page: i32, size: i32 },
	/// Filter list: /comic/filter/list?{params}&page={}&size={}
	Filter { params: &'a str, page: i32, size: i32 },
	/// Category filter: /comic/filter/list?cate={}&size={}&page={}
	Category { cate: i32, page: i32, size: i32 },
	/// Manga detail: /comic/detail/{}?channel=android
	Manga { id: &'a str },
	/// Chapter pages: /comic/chapter/{comic_id}/{chapter_id}
	ChapterPages { comic_id: &'a str, chapter_id: &'a str },
	/// Rank list: /comic/rank/list?rank_type=0&by_time={}&page={}
	Rank { by_time: i32, page: i32 },
	/// Recommend list: /comic/recommend/list
	Recommend,
	/// Tag-based filter: /comic/filter/list?theme={}&page={}&size={}
	Theme { theme_id: i64, page: i32, size: i32 },
	/// Subscribe list: /subscribe/list
	Subscribe { page: i32 },
}

impl Display for Url<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Search { keyword, page, size } => {
				write!(f, "{}/search/index?keyword={}&source=0&page={}&size={}",
					V4_API_URL, encode_uri_component(keyword), page, size)
			}
			Url::Filter { params, page, size } => {
				write!(f, "{}/comic/filter/list?{}&page={}&size={}",
					V4_API_URL, params, page, size)
			}
			Url::Category { cate, page, size } => {
				write!(f, "{}/comic/filter/list?cate={}&size={}&page={}",
					V4_API_URL, cate, size, page)
			}
			Url::Manga { id } => {
				write!(f, "{}/comic/detail/{}?channel=android", V4_API_URL, id)
			}
			Url::ChapterPages { comic_id, chapter_id } => {
				write!(f, "{}/comic/chapter/{}/{}", V4_API_URL, comic_id, chapter_id)
			}
			Url::Rank { by_time, page } => {
				write!(f, "{}/comic/rank/list?rank_type=0&by_time={}&page={}",
					V4_API_URL, by_time, page)
			}
			Url::Recommend => {
				write!(f, "{}/comic/recommend/list", V4_API_URL)
			}
			Url::Theme { theme_id, page, size } => {
				write!(f, "{}/comic/filter/list?theme={}&page={}&size={}",
					V4_API_URL, theme_id, page, size)
			}
			Url::Subscribe { page } => {
				write!(f, "{}/comic/sub/list?status=0&firstLetter=&page={}&size=50", V4_API_URL, page)
			}
		}
	}
}

impl Url<'_> {
	/// Create a Request for this URL, with auth if Enhanced Mode is active
	pub fn request(&self) -> Result<Request> {
		get_api_request(&self.to_string())
	}
}

// === HTTP Request Helpers ===
// Standardized headers and auth injection.

/// Create a GET request, attaching auth token if Enhanced Mode is active.
pub fn get_api_request(url: &str) -> Result<Request> {
	if let Some(token) = settings::get_token() {
		if settings::get_enhanced_mode() {
			auth_request(url, &token)
		} else {
			get_request(url)
		}
	} else {
		get_request(url)
	}
}

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

pub fn auth_request(url: &str, token: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Authorization", &format!("Bearer {}", token)))
}

// === API Methods ===
// Specific API implementations (Login, Check-in, UserInfo).

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

	// Success response has empty data; validate via errno only.
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
		auth_request(&url, token)?.json_owned()?;
	response
		.data
		.ok_or_else(|| aidoku::error!("Missing user info data"))
}

// === Hidden Content Scanner ===
// Iterator-based lazy loader for "Hidden" content batching.
/// Scanner for hidden content, implementing Iterator for lazy fetching
pub struct HiddenContentScanner {
	current_page: i32,
	scanned_batches: i32,
	max_batches: i32,
}

impl HiddenContentScanner {
	pub fn new(start_page: i32, max_batches: i32) -> Self {
		Self {
			current_page: start_page,
			scanned_batches: 0,
			max_batches,
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

		// Inner loop to skip empty batches (up to remaining allowance)
		// We consume our "allowance" of retries even if we skip
		while self.scanned_batches < self.max_batches {
			self.scanned_batches += 1;
			let end_page = self.current_page + 4;
			
			// Fetch 5 pages
			let requests: Vec<Request> = (self.current_page..=end_page)
				.filter_map(|p| Url::Filter { params: "sortType=1", page: p, size: 100 }.request().ok())
				.collect();

			items = Request::send_all(requests)
				.into_iter()
				.flatten()
				.filter_map(|resp| {
					resp.get_json_owned::<models::ApiResponse<models::FilterData>>()
						.ok()
						.and_then(|r| r.data)
				})
				.flat_map(|data| data.comic_list)
				.collect();

			self.current_page += 5; // Advance for next time

			if !items.is_empty() {
				batch_found = true;
				break;
			}
			// If empty, loop continues, effectively "skipping" this batch
		}

		if batch_found {
			Some(items)
		} else {
			None
		}
	}
}
