mod url;

pub use url::{
	extract_chapter_key, extract_chapter_number, extract_key, get_absolute_url, get_chapter_url,
	get_search_url,
};

use aidoku::{Result, imports::net::Request};

pub const BASE_URL: &str = "https://m.rumanhua.org";

pub fn get_request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", "Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1")
		.header("Referer", "https://m.rumanhua.org/")
		.header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
		.header("Cookie", "usepc=0"))
}
