use aidoku::{
	alloc::string::String,
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

pub const ADDRESS_KEY: &str = "rsAddress";
const REVISION_KEY: &str = "history.revision";
const TOKEN_KEY: &str = "rsToken";
const STORAGE_URL_KEY: &str = "storageUrl";
const OAUTH_URL_KEY: &str = "rsOAuthUrl";
const SHOW_HELP_KEY: &str = "showHelp";
const SAVE_SERIES_KEY: &str = "saveSeries";

fn set_defaults_string(key: &str, value: &str) {
	defaults_set(
		key,
		if value.is_empty() {
			DefaultValue::Null
		} else {
			DefaultValue::String(value.into())
		},
	);
}

pub fn get_history_revision() -> String {
	defaults_get::<String>(REVISION_KEY).unwrap_or_default()
}

pub fn set_history_revision<T: AsRef<str>>(revision: T) {
	set_defaults_string(REVISION_KEY, revision.as_ref());
}

pub fn get_token() -> String {
	defaults_get::<String>(TOKEN_KEY).unwrap_or_default()
}

pub fn set_token<T: AsRef<str>>(token: T) {
	set_defaults_string(TOKEN_KEY, token.as_ref());
}

pub fn get_address() -> String {
	defaults_get::<String>(ADDRESS_KEY).unwrap_or_default()
}

pub fn get_storage_url() -> String {
	defaults_get::<String>(STORAGE_URL_KEY).unwrap_or_default()
}

pub fn set_storage_url<T: AsRef<str>>(url: T) {
	set_defaults_string(STORAGE_URL_KEY, url.as_ref());
}

pub fn set_oauth_url<T: AsRef<str>>(url: T) {
	set_defaults_string(OAUTH_URL_KEY, url.as_ref());
}

pub fn get_show_help() -> bool {
	defaults_get::<bool>(SHOW_HELP_KEY).unwrap_or(true)
}

pub fn get_save_series() -> bool {
	defaults_get::<bool>(SAVE_SERIES_KEY).unwrap_or(true)
}
