use aidoku::{alloc::String, imports::defaults::defaults_get};

pub fn get_api_url() -> String {
	defaults_get::<String>("apiUrl").unwrap_or_default()
}

pub fn get_image_server_url() -> String {
	defaults_get::<String>("imageServerUrl").unwrap_or_default()
}
