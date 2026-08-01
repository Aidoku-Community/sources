use crate::settings::base_url;
use aidoku::{
	Result,
	alloc::{String, Vec},
	helpers::uri::encode_uri_component,
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		html::Document,
		net::{Request, Response},
	},
	prelude::*,
};

const COOKIE_KEY: &str = "login.cookie";
const LOGGED_IN_KEY: &str = "login.ok";
const JUST_LOGGED_IN_KEY: &str = "login.just";
const USERNAME_KEY: &str = "login.username";
const STORED_USERNAME_KEY: &str = "desu.username";

pub fn is_logged_in() -> bool {
	defaults_get::<bool>(LOGGED_IN_KEY).unwrap_or(false)
		|| defaults_get::<String>(COOKIE_KEY).is_some_and(|c| c.contains("xf_user="))
}

pub fn stored_username() -> Option<String> {
	defaults_get::<String>(STORED_USERNAME_KEY)
		.or_else(|| defaults_get::<String>(USERNAME_KEY))
		.filter(|s| !s.is_empty())
}

fn store_username(username: &str) {
	defaults_set(STORED_USERNAME_KEY, DefaultValue::String(username.into()));
}

pub fn set_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
}

pub fn take_just_logged_in() -> bool {
	let flag = defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false);
	if flag {
		defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
	}
	flag
}

pub fn logout() {
	defaults_set(COOKIE_KEY, DefaultValue::Null);
	defaults_set(LOGGED_IN_KEY, DefaultValue::Null);
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
	defaults_set(STORED_USERNAME_KEY, DefaultValue::Null);
}

fn store_cookie_header(header: &str) {
	defaults_set(COOKIE_KEY, DefaultValue::String(header.into()));
}

fn set_logged_in(value: bool) {
	if value {
		defaults_set(LOGGED_IN_KEY, DefaultValue::Bool(true));
	} else {
		defaults_set(LOGGED_IN_KEY, DefaultValue::Null);
	}
}

fn merge_xf_cookies(existing: &str, set_cookie: &str) -> String {
	let mut map: Vec<(String, String)> = Vec::new();
	for part in existing.split(';') {
		let part = part.trim();
		if let Some((name, value)) = part.split_once('=')
			&& (name == "xf_user" || name == "xf_session")
		{
			map.retain(|(n, _)| n != name);
			map.push((name.into(), value.into()));
		}
	}
	for segment in set_cookie.split('\n') {
		let pair = segment.split(';').next().unwrap_or("").trim();
		if let Some((name, value)) = pair.split_once('=')
			&& (name == "xf_user" || name == "xf_session")
			&& !value.is_empty()
		{
			map.retain(|(n, _)| n != name);
			map.push((name.into(), value.into()));
		}
	}
	map.into_iter()
		.map(|(n, v)| format!("{n}={v}"))
		.collect::<Vec<_>>()
		.join("; ")
}

fn capture_cookies(response: &Response) {
	let existing = defaults_get::<String>(COOKIE_KEY).unwrap_or_default();
	if let Some(set_cookie) = response.get_header("Set-Cookie") {
		let merged = merge_xf_cookies(&existing, &set_cookie);
		if !merged.is_empty() {
			store_cookie_header(&merged);
		}
	}
}

pub trait AuthedRequest {
	fn authed(self) -> Self;
}

impl AuthedRequest for Request {
	fn authed(self) -> Self {
		if let Some(cookie) = defaults_get::<String>(COOKIE_KEY).filter(|c| !c.is_empty()) {
			self.header("Cookie", &cookie)
		} else {
			self
		}
	}
}

fn get_base_url() -> String {
	base_url()
}

fn base_headers(request: Request) -> Request {
	request
		.authed()
		.header("User-Agent", "Aidoku")
		.header("Referer", get_base_url().as_str())
}

fn request_html(url: &str) -> Result<Document> {
	let response = base_headers(Request::get(url)?).send()?;
	capture_cookies(&response);
	Ok(response.get_html()?)
}

pub fn login(username: &str, password: &str) -> Result<bool> {
	if username.is_empty() || password.is_empty() {
		return Ok(false);
	}

	logout();

	let base = get_base_url();
	let login_page = request_html(&format!("{base}/login/"))?;
	let token = login_page
		.select_first("input[name=_xfToken]")
		.and_then(|el| el.attr("value"))
		.unwrap_or_default();

	let body = [
		("login", username),
		("password", password),
		("remember", "1"),
		("register", "0"),
		("cookie_check", "1"),
		("_xfToken", token.as_str()),
		("redirect", base.as_str()),
	]
	.into_iter()
	.map(|(k, v)| format!("{k}={}", encode_uri_component(v)))
	.collect::<Vec<_>>()
	.join("&");

	let response = base_headers(Request::post(format!("{base}/login/login"))?)
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Referer", &format!("{base}/login/"))
		.body(body.as_bytes())
		.send()?;
	capture_cookies(&response);

	let verify = base_headers(Request::get(format!("{base}/ranobe/?order_by=updated"))?).send()?;
	capture_cookies(&verify);
	let ok = verify.status_code() < 400;
	set_logged_in(ok);
	if ok {
		store_username(username);
		set_just_logged_in();
	} else {
		logout();
	}
	Ok(ok)
}

pub fn require_login() -> Result<()> {
	if is_logged_in() {
		Ok(())
	} else {
		bail!("Войдите в аккаунт Desu в настройках источника")
	}
}
