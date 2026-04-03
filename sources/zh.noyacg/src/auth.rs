use aidoku::{
	Result,
	alloc::String,
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	imports::net::Request,
	imports::std::current_date,
	prelude::*,
};

use crate::helpers::{build_form_body, get_base_url};
use crate::models::{LoginResp, SigninRecordResp};

const CREDENTIALS_KEY: &str = "credentials";
const JUST_LOGGED_IN_KEY: &str = "justLoggedIn";
const LAST_SIGNIN_DAY_KEY: &str = "lastSigninDay";
const UTC8_OFFSET: i64 = 28800;
const SECS_PER_DAY: i64 = 86400;

pub fn store_credentials(username: &str, password: &str) {
	let value = format!("{}\n{}", username, password);
	defaults_set(CREDENTIALS_KEY, DefaultValue::String(value));
}

pub fn get_credentials() -> Option<(String, String)> {
	let raw = defaults_get::<String>(CREDENTIALS_KEY).filter(|v| !v.is_empty())?;
	let (user, pass) = raw.split_once('\n')?;
	Some((user.into(), pass.into()))
}

pub fn clear_credentials() {
	defaults_set(CREDENTIALS_KEY, DefaultValue::Null);
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
}

pub fn is_logged_in() -> bool {
	defaults_get::<String>(CREDENTIALS_KEY)
		.filter(|v| !v.is_empty())
		.is_some()
}

pub fn set_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
}

pub fn is_just_logged_in() -> bool {
	defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false)
}

pub fn clear_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
}

pub fn do_login(username: &str, password: &str) -> Result<bool> {
	let base_url = get_base_url();
	let body = build_form_body(&[("user", username), ("pass", password)]);
	let resp: LoginResp = Request::post(format!("{base_url}/api/login"))?
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Referer", &format!("{base_url}/"))
		.body(body.as_bytes())
		.json_owned()?;
	Ok(resp.status.as_deref() == Some("ok"))
}

pub fn ensure_session() -> Result<()> {
	let Some((username, password)) = get_credentials() else {
		return Ok(());
	};
	if !do_login(&username, &password)? {
		clear_credentials();
		bail!("登入已過期，請重新登入");
	}
	Ok(())
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
	let base_url = get_base_url();
	let referer = format!("{base_url}/");
	// check server record first to avoid a redundant sign request
	let Ok(req) = Request::post(format!("{base_url}/api/v4/signin/record")) else {
		return;
	};
	let Ok(record) = req
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Referer", &referer)
		.json_owned::<SigninRecordResp>()
	else {
		return;
	};
	if record.today == Some(true) {
		defaults_set(LAST_SIGNIN_DAY_KEY, DefaultValue::String(today));
		return;
	}
	let Ok(req) = Request::post(format!("{base_url}/api/v4/signin/sign")) else {
		return;
	};
	if req
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Referer", &referer)
		.send()
		.is_ok()
	{
		defaults_set(LAST_SIGNIN_DAY_KEY, DefaultValue::String(today));
	}
}
