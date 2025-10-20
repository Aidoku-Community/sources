use aidoku::{
	AidokuError, Result,
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

pub fn get_username() -> Result<String> {
	defaults_get::<String>("username").ok_or(AidokuError::message("Username not set"))
}

pub fn get_password() -> Result<String> {
	defaults_get::<String>("password").ok_or(AidokuError::message("Password not set"))
}

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
