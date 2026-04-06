use crate::{ACCOUNT_API, SIGN_API, USER_AGENT, models, settings};
use aidoku::{
	Result,
	alloc::{String, Vec, format},
	helpers::uri::encode_uri_component,
	imports::net::{Request, Response},
	serde::de::DeserializeOwned,
};

pub mod urls {
	use crate::V4_API_URL;
	use aidoku::{
		alloc::{String, format},
		helpers::uri::encode_uri_component,
	};

	pub fn search(keyword: &str, page: i32) -> String {
		let keyword = encode_uri_component(keyword);
		format!("{V4_API_URL}/search/index?keyword={keyword}&source=0&page={page}")
	}

	pub fn search_sized(keyword: &str, page: i32, size: i32) -> String {
		let keyword = encode_uri_component(keyword);
		format!("{V4_API_URL}/search/index?keyword={keyword}&source=0&page={page}&size={size}")
	}

	pub fn filter(query_string: &str, page: i32) -> String {
		format!("{V4_API_URL}/comic/filter/list?{query_string}&page={page}")
	}

	pub fn filter_latest_sized(page: i32, size: i32) -> String {
		format!("{V4_API_URL}/comic/filter/list?sortType=1&page={page}&size={size}")
	}

	pub fn filter_cate(cate: i64, page: i32, size: i32) -> String {
		format!("{V4_API_URL}/comic/filter/list?cate={cate}&size={size}&page={page}")
	}

	pub fn detail(id: i64) -> String {
		format!("{V4_API_URL}/comic/detail/{id}?channel=android")
	}

	pub fn rank(by_time: i32, page: i32) -> String {
		format!("{V4_API_URL}/comic/rank/list?rank_type=0&by_time={by_time}&page={page}")
	}

	pub fn recommend() -> String {
		format!("{V4_API_URL}/comic/recommend/list")
	}

	pub fn chapter(comic_id: &str, chapter_id: &str) -> String {
		format!("{V4_API_URL}/comic/chapter/{comic_id}/{chapter_id}")
	}

	pub fn filter_theme(theme_id: i64, page: i32) -> String {
		format!("{V4_API_URL}/comic/filter/list?theme={theme_id}&page={page}")
	}

	pub fn classify() -> String {
		format!("{V4_API_URL}/comic/filter/classify")
	}

	pub fn sub_list(page: i32) -> String {
		format!("{V4_API_URL}/comic/sub/list?status=0&firstLetter=&page={page}&size=50")
	}

	pub fn manga_news() -> String {
		let news_url = crate::NEWS_URL;
		format!("{news_url}/manhuaqingbao")
	}
}

pub fn resolve_url(url: &str) -> String {
	if settings::get_use_proxy()
		&& let Some(proxy) = settings::get_proxy_url()
	{
		let encoded = encode_uri_component(url);
		return format!("{proxy}/?url={encoded}");
	}
	url.into()
}

const PROXY_BLOCKED_URLS: &[&str] = &[
//	NEWS_URL,                       // Banner
//	SIGN_API,                       // User Info API
//	V4_API_URL,                     // Main API (Content, Listings)
//	WEB_URL,                        // Web View
//	ACCOUNT_API,                    // Login API
];

pub fn should_block(url: &str) -> bool {
	settings::get_use_proxy()
		&& PROXY_BLOCKED_URLS
			.iter()
			.any(|blocked| url.starts_with(blocked))
}

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
	let req = get_request(url)?;
	Ok(match token {
		Some(t) => req.header("Authorization", &format!("Bearer {t}")),
		None => req,
	})
}

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
	let resp: models::ApiResponse<T> = req.json_owned()?;

	if resp.errno.unwrap_or(0) == 99
		&& let Ok(Some(new_token)) = try_refresh_token()
	{
		return auth_request(url, Some(&new_token))?.json_owned();
	}
	Ok(resp)
}

pub fn login(username: &str, password: &str) -> Result<Option<String>> {
	let password_hash = md5_hex(password);
	let url = format!("{ACCOUNT_API}/login/passwd");
	let body = format!(
		"username={}&passwd={password_hash}",
		encode_uri_component(username),
	);

	let response: models::ApiResponse<models::LoginData> =
		post_request(&url)?.body(body.as_bytes()).json_owned()?;

	if response.errno.unwrap_or(-1) != 0 {
		return Ok(None);
	}

	Ok(response.data.and_then(|d| d.user).and_then(|u| u.token))
}

fn check_in(token: &str) -> Result<bool> {
	let url = format!("{SIGN_API}/task/sign_in");
	let response: models::ApiResponse<aidoku::serde::de::IgnoredAny> = post_request(&url)?
		.header("Authorization", &format!("Bearer {token}"))
		.json_owned()?;
	Ok(response.errno.unwrap_or(-1) == 0)
}

pub fn get_user_info(token: &str) -> Result<models::UserInfoData> {
	let url = format!("{SIGN_API}/userInfo/get");
	let response: models::ApiResponse<models::UserInfoData> =
		send_authed_request(&url, Some(token))?;
	response.data.ok_or_else(|| aidoku::error!("用户信息缺失"))
}

pub fn refresh_user_profile(token: &str) -> Result<()> {
	let info_data = get_user_info(token)?;
	if let Some(info) = info_data.user_info {
		let level = info.level.unwrap_or(0) as i32;
		let is_sign = info.is_sign.unwrap_or(false);
		settings::set_user_cache(level, is_sign);
	}
	Ok(())
}

fn claim_pending_rewards(token: &str) {
	let url = format!("{SIGN_API}/task/list");
	let Ok(resp) = send_authed_request::<models::TaskListData>(&url, Some(token)) else {
		return;
	};
	let Some(task) = resp.data.and_then(|d| d.task) else {
		return;
	};

	let claimable = task
		.day_task
		.iter()
		.flatten()
		.chain(
			task.sum_sign_task
				.iter()
				.flat_map(|s| s.list.iter().flatten()),
		)
		.filter(|t| t.status == Some(2));

	for item in claimable {
		let reward_url = format!("{SIGN_API}/task/get_reward?task_id={}", item.id);
		let _ = send_authed_request::<aidoku::serde::de::IgnoredAny>(&reward_url, Some(token));
	}
}

pub fn perform_silent_updates() {
	let Some(token) = settings::get_token() else {
		return;
	};

	if settings::get_auto_checkin()
		&& !settings::has_checkin_flag()
		&& !should_block(crate::SIGN_API)
	{
		let _ = refresh_user_profile(&token);
		// re-read in case token was refreshed via errno=99 retry
		let token = settings::get_token().unwrap_or(token);

		let already_signed = settings::get_user_cache().is_some_and(|c| c.is_sign);
		let signed = already_signed || check_in(&token).unwrap_or(false);

		if signed {
			settings::set_last_checkin();
			claim_pending_rewards(&token);
			let _ = refresh_user_profile(&token);
		}
		return;
	}

	if settings::is_cache_stale() && !should_block(crate::SIGN_API) {
		let _ = refresh_user_profile(&token);
	}
}

pub struct RequestBatch {
	requests: Vec<Request>,
	index_map: Vec<usize>,
	total_slots: usize,
}

impl RequestBatch {
	pub fn new() -> Self {
		Self {
			requests: Vec::new(),
			index_map: Vec::new(),
			total_slots: 0,
		}
	}

	pub fn get(&mut self, url: &str) -> Result<usize> {
		if should_block(url) {
			return Ok(self.add_if(None));
		}
		Ok(self.add(get_request(url)?))
	}

	pub fn auth(&mut self, url: &str, token: Option<&str>) -> Result<usize> {
		if should_block(url) {
			return Ok(self.add_if(None));
		}
		Ok(self.add(auth_request(url, token)?))
	}

	fn add(&mut self, req: Request) -> usize {
		self.add_if(Some(req))
	}

	fn add_if(&mut self, req: Option<Request>) -> usize {
		let slot = self.total_slots;
		if let Some(r) = req {
			self.requests.push(r);
			self.index_map.push(slot);
		}
		self.total_slots += 1;
		slot
	}

	pub fn add_unless_blocked(&mut self, url: &str) -> usize {
		let req = if should_block(url) {
			None
		} else {
			get_request(url).ok()
		};
		self.add_if(req)
	}

	pub fn send_all(self) -> Vec<Option<Response>> {
		let responses = Request::send_all(self.requests);
		let mut result: Vec<Option<Response>> = (0..self.total_slots).map(|_| None).collect();
		for (resp, slot) in responses.into_iter().zip(self.index_map) {
			if let Ok(r) = resp {
				result[slot] = Some(r);
			}
		}
		result
	}
}
