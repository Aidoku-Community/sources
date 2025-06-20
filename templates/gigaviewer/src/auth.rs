use aidoku::{
	alloc::String,
	helpers::uri::encode_uri_component,
	imports::{
		defaults::{defaults_get, defaults_set, DefaultValue},
		net::Request,
	},
	prelude::*,
	Result,
};

static EMAIL_KEY: &str = "login.username";
// static PASSWORD_KEY: &str = "login.password";
static COOKIE_KEY: &str = "login.cookie";

pub fn login(base_url: &str, email: &str, password: &str) -> Result<bool> {
	let url = format!("{base_url}/user_account/login");
	let body = format!(
		"email_address={}&password={}&return_location_path=/",
		encode_uri_component(email),
		encode_uri_component(password)
	);
	let req = Request::post(&url)?
		.header("x-requested-with", "XMLHttpRequest")
		.body(&body);
	let res = req.send()?;

	let status_code = res.status_code();
	if status_code == 200 {
		let cookie = res.get_header("Set-Cookie");
		if let Some(cookie) = cookie {
			defaults_set(COOKIE_KEY, DefaultValue::String(cookie));
			Ok(true)
		} else {
			Ok(false)
		}
	} else {
		Ok(false)
	}
}

pub fn logout() {
	defaults_set(COOKIE_KEY, DefaultValue::Null);
}

pub fn is_logged_in() -> bool {
	let cookie = defaults_get::<String>(EMAIL_KEY);
	cookie.is_some()
}

pub trait AuthedRequest {
	fn authed(self) -> Self;
}

impl AuthedRequest for Request {
	fn authed(self) -> Self {
		let cookie = defaults_get::<String>(COOKIE_KEY);
		if let Some(cookie) = cookie {
			self.header("Cookie", &cookie)
		} else {
			self
		}
	}
}
