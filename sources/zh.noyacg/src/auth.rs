use aidoku::{
	Result,
	alloc::{String, string::ToString},
	helpers::uri::QueryParameters,
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	imports::std::current_date,
	prelude::*,
};

use crate::helpers::{api_post, fetch_json, get_adult_mode};
use crate::models::{LoginResp, SigninRecordResp};

const USERNAME_KEY: &str = "login.username";
const PASSWORD_KEY: &str = "login.password";
const LAST_SIGNIN_DAY_KEY: &str = "lastSigninDay";
const UTC8_OFFSET: i64 = 28800;
const SECS_PER_DAY: i64 = 86400;

pub fn is_logged_in() -> bool {
	defaults_get::<String>(USERNAME_KEY).is_some()
}

pub fn login(username: &str, password: &str) -> Result<bool> {
	let mut body = QueryParameters::new();
	body.push("user", Some(username));
	body.push("pass", Some(password));
	let resp: LoginResp =
		api_post("/api/login", &body.to_string(), get_adult_mode())?.json_owned()?;
	Ok(resp.status.as_deref() == Some("ok"))
}

pub fn relogin() -> Result<()> {
	let (Some(username), Some(password)) = (
		defaults_get::<String>(USERNAME_KEY),
		defaults_get::<String>(PASSWORD_KEY),
	) else {
		bail!("請先登入以檢視內容");
	};
	if !login(&username, &password)? {
		bail!("登入已過期，請重新登入");
	}
	Ok(())
}

pub fn logout() {
	if let Ok(request) = api_post("/api/logout", "", get_adult_mode()) {
		_ = request.send();
	}
}

fn today_utc8() -> String {
	let day = (current_date() + UTC8_OFFSET) / SECS_PER_DAY;
	format!("{day}")
}

pub fn try_daily_signin() {
	if !is_logged_in() {
		return;
	}
	if !defaults_get::<bool>("auto_signin").unwrap_or(false) {
		return;
	}
	let today = today_utc8();
	if defaults_get::<String>(LAST_SIGNIN_DAY_KEY).as_deref() == Some(today.as_str()) {
		return;
	}
	let adult = get_adult_mode();
	let Ok(record) =
		fetch_json::<SigninRecordResp>(|| api_post("/api/v4/signin/record", "", adult))
	else {
		return;
	};
	if record.today == Some(true) {
		defaults_set(LAST_SIGNIN_DAY_KEY, DefaultValue::String(today));
		return;
	}
	let Ok(request) = api_post("/api/v4/signin/sign", "", adult) else {
		return;
	};
	if request.send().is_ok() {
		defaults_set(LAST_SIGNIN_DAY_KEY, DefaultValue::String(today));
	}
}
