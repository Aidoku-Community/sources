use crate::net::base_url;
use aidoku::{
	Result,
	alloc::{String, format},
	bail, error,
	helpers::uri::encode_uri_component,
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		net::Request,
		std::current_date,
	},
	serde::Deserialize,
};

// 登录态存储键。`login.username`/`login.password` 由 App 在 basic 登录时写入、登出时清除；
// 我们自己的 token 存在 `auth.token`，并在收到 login 通知且非刚登录时手动清除（登出）。
const LOGIN_USERNAME_KEY: &str = "login.username";
const TOKEN_KEY: &str = "auth.token";
const JUST_LOGGED_IN_KEY: &str = "auth.justLoggedIn";

pub fn token() -> Option<String> {
	defaults_get::<String>(TOKEN_KEY).filter(|t| !t.is_empty())
}

/// App 登出只会清掉 login.username/password，token 需要我们自己跟进；
/// 因此判断登录态必须以 App 存储的用户名为准。
pub fn is_logged_in() -> bool {
	defaults_get::<String>(LOGIN_USERNAME_KEY).is_some_and(|u| !u.is_empty()) && token().is_some()
}

pub fn clear_auth() {
	defaults_set(TOKEN_KEY, DefaultValue::Null);
	// 换号/登出时清收藏状态缓存，避免新账号看到旧账号的按钮状态
	crate::favorites::clear_all_state();
}

pub fn set_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
}

pub fn take_just_logged_in() -> bool {
	defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false)
}

pub fn clear_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
}

/// 网站前端登录算法：password 字段 = base64(明文密码 + "-" + salt)，salt 为 6 位整数。
pub fn encode_password(password: &str, salt: i64) -> Result<String> {
	use base64::{Engine, engine::general_purpose::STANDARD};
	Ok(STANDARD.encode(format!("{password}-{salt}")))
}

#[derive(Deserialize)]
struct LoginResponse {
	code: i32,
	#[serde(default)]
	message: Option<String>,
	#[serde(default)]
	results: Option<LoginResults>,
}

#[derive(Deserialize)]
struct LoginResults {
	token: String,
}

/// 用账号密码向网站登录，成功时保存 token。
pub fn login(username: &str, password: &str) -> Result<()> {
	let salt = current_date().rem_euclid(900_000) + 100_000;
	let encoded = encode_password(password, salt)?;
	let body = format!(
		"username={}&password={}&salt={salt}&platform=2&version=1.0.0&source=copyweb",
		encode_uri_component(username),
		encode_uri_component(&encoded),
	);

	let url = format!("{}/api/v1/login", base_url()?);
	let response = Request::post(&url)?
		.header(
			"Content-Type",
			"application/x-www-form-urlencoded;charset=UTF-8",
		)
		.header("X-Requested-With", "XMLHttpRequest")
		.body(body)
		.send()?;

	let body = response.get_string()?;
	let login: LoginResponse =
		serde_json::from_str(&body).map_err(|_| error!("登錄響應解析失敗"))?;
	if login.code != 200 {
		let message = login
			.message
			.unwrap_or_else(|| format!("HTTP {}", response.status_code()));
		bail!("登錄失敗：{message}");
	}
	let results = login.results.ok_or_else(|| error!("登錄響應缺少 token"))?;

	defaults_set(TOKEN_KEY, DefaultValue::String(results.token));
	Ok(())
}

/// token 失效（401）时利用 App 保存的凭据静默重登一次。
pub fn try_relogin() -> bool {
	let Some(username) = defaults_get::<String>(LOGIN_USERNAME_KEY).filter(|u| !u.is_empty())
	else {
		return false;
	};
	let Some(password) = defaults_get::<String>("login.password").filter(|p| !p.is_empty()) else {
		return false;
	};
	login(&username, &password).is_ok()
}

pub trait AuthedRequest {
	fn authed(self) -> Result<Request>;
}

impl AuthedRequest for Request {
	fn authed(self) -> Result<Request> {
		let token = token().ok_or_else(|| error!("請先在設置中登錄"))?;
		Ok(self.header("Authorization", &format!("Token {token}")))
	}
}
