use aidoku::imports::defaults::defaults_get;
use alloc::format;
use alloc::string::String;

const SHOW_PAID_INFO_KEY: &str = "showPaidInfo";
const ENGLISH_TITLES_KEY: &str = "englishTitles";
const ACCESS_TOKEN_SETTING_KEY: &str = "accessToken";

pub const SITE_URL: &str = "https://remanga.org";
pub const API_V1: &str = "https://api.remanga.org/api";
pub const API_V2: &str = "https://remanga.org/api/v2";

/// Shared UA for Remanga API and image requests.
pub const USER_AGENT: &str = "Mozilla/5.0 (compatible; Aidoku)";

/// When enabled, locked chapters show price / free-from date in the title.
pub fn show_paid_info() -> bool {
	defaults_get::<bool>(SHOW_PAID_INFO_KEY).unwrap_or(true)
}

/// Prefer secondary (usually EN) title when present.
pub fn english_titles() -> bool {
	defaults_get::<bool>(ENGLISH_TITLES_KEY).unwrap_or(false)
}

/// Manual Bearer token from settings (fallback when WebView login fails).
pub fn settings_access_token() -> Option<String> {
	defaults_get::<String>(ACCESS_TOKEN_SETTING_KEY).filter(|s| !s.trim().is_empty())
}

/// Turns a relative media path into an absolute Remanga URL.
pub fn media_url(path: &str) -> String {
	if path.starts_with("http://") || path.starts_with("https://") {
		path.into()
	} else if path.starts_with('/') {
		format!("{SITE_URL}{path}")
	} else {
		format!("{SITE_URL}/{path}")
	}
}
