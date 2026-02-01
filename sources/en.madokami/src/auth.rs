use aidoku::{
	Result,
	alloc::String,
	imports::{defaults::defaults_get, net::Request},
	prelude::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};

static USERNAME_KEY: &str = "login.username";
static PASSWORD_KEY: &str = "login.password";

pub trait AuthedRequest {
	fn authed(self) -> Result<Request>;
}

impl AuthedRequest for Request {
	fn authed(self) -> Result<Self> {
		let error = || error!("Login required.");
		let username = defaults_get::<String>(USERNAME_KEY).ok_or_else(error)?;
		let password = defaults_get::<String>(PASSWORD_KEY).ok_or_else(error)?;
		Ok(self.header("Authorization", &header(&username, &password)))
	}
}

pub fn header(username: &str, password: &str) -> String {
	let encoded = STANDARD.encode(format!("{username}:{password}"));
	format!("Basic {encoded}")
}
