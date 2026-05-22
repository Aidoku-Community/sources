use crate::{API_URL, models::*};
use aidoku::{
	HashMap, Result,
	alloc::string::String,
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set, defaults_set_data},
		net::Request,
	},
	prelude::*,
};

const AUTH_KEY: &str = "auth";

pub fn is_logged_in() -> bool {
	defaults_get::<LoginStatus>(AUTH_KEY).is_some()
}

pub fn is_subscribed() -> bool {
	defaults_get::<LoginStatus>(AUTH_KEY).is_some_and(|s| s.is_subscribed)
}

pub fn handle_login(cookies: HashMap<String, String>) -> Result<bool> {
	let Some(refresh_token) = cookies.get("refresh_token") else {
		return Ok(false);
	};
	let Ok(status) = refresh(refresh_token) else {
		bail!("Failed to authenticate");
	};
	defaults_set_data(AUTH_KEY, status);
	Ok(true)
}

pub fn logout() {
	defaults_set(AUTH_KEY, DefaultValue::Null);
}

pub fn get_access_token() -> Result<String> {
	Ok(get_login_status()?.access_token)
}

pub fn get_login_status() -> Result<LoginStatus> {
	let old_status = defaults_get::<LoginStatus>(AUTH_KEY).ok_or(error!("Not logged in"))?;
	let status = refresh(&old_status.refresh_token)?;
	defaults_set_data(AUTH_KEY, status.clone());
	Ok(status)
}

fn refresh(refresh_token: &str) -> Result<LoginStatus> {
	let mut res: RefreshResponse = Request::post(format!("{API_URL}/auth/refresh"))?
		.header("Content-Type", "application/json")
		.body(format!("{{\"refresh_token\":\"{refresh_token}\"}}"))
		.json_owned()?;
	if res.data.refresh_token.is_none() {
		res.data.refresh_token = Some(refresh_token.into());
	}
	Ok(LoginStatus::from(res))
}
