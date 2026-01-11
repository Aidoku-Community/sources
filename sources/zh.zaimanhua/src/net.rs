use crate::models;
use aidoku::{
	Result,
	alloc::{String, format},
	imports::net::Request,
};

pub const ACCOUNT_API: &str = "https://account-api.zaimanhua.com/v1/";
pub const SIGN_API: &str = "https://i.zaimanhua.com/lpi/v1/";
pub const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 10) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

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
