use aidoku::{Result, alloc::String, imports::defaults::defaults_get, prelude::*};

pub fn get_base_url() -> Result<String> {
	let url: String = defaults_get::<String>("baseUrl").ok_or(error!("Missing baseUrl"))?;
	Ok(url.trim_end_matches('/').into())
}

pub fn get_auth_mode() -> String {
	match defaults_get::<String>("authMode") {
		Some(v) if !v.is_empty() => v,
		_ => "auto".into(),
	}
}

pub fn get_credentials() -> Option<(String, String)> {
	let user: String = defaults_get::<String>("credentials.username")?;
	let pass: String = defaults_get::<String>("credentials.password")?;

	if user.is_empty() {
		return None;
	}
	Some((user, pass))
}
