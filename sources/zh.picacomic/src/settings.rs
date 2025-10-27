use aidoku::{
	Result,
	alloc::{String, Vec, string::ToString},
	error,
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

pub fn get_app_channel() -> String {
	defaults_get::<String>("appChannel").unwrap_or("3".into())
}

pub fn get_image_quality() -> String {
	defaults_get::<String>("imageQuality").unwrap_or("original".into())
}

pub fn get_blocklist() -> Vec<String> {
	defaults_get::<Vec<String>>("blockGenres").unwrap_or_default()
}

pub fn get_list_viewer() -> bool {
	defaults_get("isListView").unwrap_or(false)
}

pub fn get_username() -> Result<String> {
	defaults_get::<String>("username").ok_or_else(|| error!("Please log in first"))
}

pub fn get_password() -> Result<String> {
	defaults_get::<String>("password").ok_or_else(|| error!("Please log in first"))
}

pub fn set_username(username: &str) -> Result<()> {
	defaults_set("username", DefaultValue::String(username.to_string()));
	Ok(())
}

pub fn set_password(password: &str) -> Result<()> {
	defaults_set("password", DefaultValue::String(password.to_string()));
	Ok(())
}
