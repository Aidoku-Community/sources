use aidoku::alloc::{
	format,
	string::{String, ToString},
	vec::Vec,
};

pub struct Url;

impl Url {
	const BASE_PATH: &'static str = "/v2";

	fn append_query_params(base_url: String, params: &[(&str, &str)]) -> String {
		let mut url = base_url;
		if !params.is_empty() {
			url.push('?');
			for (i, (key, value)) in params.iter().enumerate() {
				if i > 0 {
					url.push('&');
				}
				url.push_str(key);
				url.push('=');
				url.push_str(value);
			}
		}
		url
	}

	pub fn manga_search_with_params(base_url: &str, params: Vec<(String, String)>) -> String {
		let url = format!("{}{}/books", base_url, Self::BASE_PATH);
		let ref_array: Vec<(&str, &str)> = params
			.iter()
			.map(|(key, value)| (key.as_str(), value.as_str()))
			.collect();
		Self::append_query_params(url, &ref_array)
	}

	pub fn manga_details(base_url: &str, id: &str) -> String {
		format!("{}{}/books/{}", base_url, Self::BASE_PATH, id)
	}

	pub fn manga_branches(base_url: &str, book_id: &str, page: i32) -> String {
		let url = format!("{}{}/branches", base_url, Self::BASE_PATH);
		let params = [
			("moderationStatus", "APPROVED"),
			("bookId", book_id),
			("page", &page.to_string()),
			("size", "20"),
		];
		Self::append_query_params(url, &params)
	}

	pub fn manga_chapters(base_url: &str, book_id: &str) -> String {
		let url = format!("{}{}/chapters", &base_url, Self::BASE_PATH);
		let params = [("moderationStatus", "APPROVED"), ("bookId", book_id)];
		Self::append_query_params(url, &params)
	}

	pub fn chapter_page(base_url: &str, chapter_id: &str) -> String {
		format!("{}{}/chapters/{}", base_url, Self::BASE_PATH, chapter_id)
	}

	pub fn labels(base_url: &str) -> String {
		format!("{}{}/labels", base_url, Self::BASE_PATH)
	}
}
