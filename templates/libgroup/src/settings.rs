use aidoku::{alloc::String, imports::defaults::defaults_get};

const BASE_URL_KEY: &str = "baseUrl";

const API_URL_KEY: &str = "apiUrl";
const IMAGE_SERVER_KEY: &str = "imageServerUrl";
const COVER_QUALITY_KEY: &str = "coverQuality";

const DEFAULT_API_URL: &str = "https://api.imglib.info";
const DEFAULT_IMAGE_SERVER: &str = "compress";
const DEFAULT_COVER_QUALITY: &str = "default";

/// Get the base URL
pub fn get_base_url() -> String {
	defaults_get::<String>(BASE_URL_KEY).unwrap_or_default()
}

/// Get the API base URL
pub fn get_api_url() -> String {
	defaults_get::<String>(API_URL_KEY).unwrap_or_else(|| DEFAULT_API_URL.into())
}

/// Get the image server base URL
pub fn get_image_server_url() -> String {
	defaults_get::<String>(IMAGE_SERVER_KEY).unwrap_or_else(|| DEFAULT_IMAGE_SERVER.into())
}

/// Get the cover quality setting
pub fn get_cover_quality_url() -> String {
	defaults_get::<String>(COVER_QUALITY_KEY).unwrap_or_else(|| DEFAULT_COVER_QUALITY.into())
}
