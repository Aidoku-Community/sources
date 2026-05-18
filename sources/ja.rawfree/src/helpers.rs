use aidoku::{alloc::string::String, imports::defaults::defaults_get};

const BASE_URL: &str = "https://rawfree.gg";

pub fn get_base_url() -> String {
	let base_url = defaults_get::<String>("baseUrl");
	match base_url {
		Some(url) if !url.is_empty() => url,
		_ => BASE_URL.into(),
	}
}

pub fn clean_title(title: String) -> String {
	let suffixes = ["(Raw – Free)", "(Raw - Free)"];
	for suffix in suffixes {
		if let Some(clean) = title.strip_suffix(suffix) {
			return clean.into();
		}
	}
	title
}
