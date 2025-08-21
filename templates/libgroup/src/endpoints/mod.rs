use aidoku::{
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
};

/// URL builder for API endpoints
pub struct Url;

impl Url {
	const BASE_PATH: &'static str = "/api";

	/// Build manga search URL
	pub fn manga_search(base_url: &str) -> String {
		format!(
			"{}{}/manga",
			Self::normalize_base(base_url),
			Self::BASE_PATH
		)
	}

	/// Build manga details URL
	pub fn manga_details(base_url: &str, slug_url: &str) -> String {
		format!(
			"{}{}/manga/{}",
			Self::normalize_base(base_url),
			Self::BASE_PATH,
			slug_url
		)
	}

	/// Build manga page URL
	pub fn manga_page(base_url: &str, slug_url: &str) -> String {
		format!("{}/ru/manga/{}", Self::normalize_base(base_url), slug_url)
	}

	/// Build manga chapters URL
	pub fn manga_chapters(base_url: &str, slug_url: &str) -> String {
		format!(
			"{}{}/manga/{}/chapters",
			Self::normalize_base(base_url),
			Self::BASE_PATH,
			slug_url
		)
	}

	/// Build chapter pages URL
	pub fn chapter_pages(base_url: &str, slug_url: &str) -> String {
		format!(
			"{}{}/manga/{}/chapter",
			Self::normalize_base(base_url),
			Self::BASE_PATH,
			slug_url
		)
	}

	/// Build chapter read URL
	pub fn chapter_page(
		base_url: &str,
		slug_url: &str,
		volume_number: Option<f32>,
		chapter_number: Option<f32>,
		branch_id: Option<i32>,
		user_id: &Option<i32>,
	) -> String {
		let mut query_params = Vec::new();

		if let Some(id) = branch_id {
			query_params.push(format!("bid={id}"));
		}

		if let Some(id) = user_id {
			query_params.push(format!("ui={id}"));
		}

		let query_string = if query_params.is_empty() {
			String::new()
		} else {
			format!("?{}", query_params.join("&"))
		};

		format!(
			"{}/ru/{}/read/v{}/c{}{}",
			Self::normalize_base(base_url),
			slug_url,
			volume_number.unwrap_or_default(),
			chapter_number.unwrap_or_default(),
			query_string
		)
	}

	/// Build manga covers URL
	pub fn manga_covers(base_url: &str, slug_url: &str) -> String {
		format!(
			"{}{}/manga/{}/covers",
			Self::normalize_base(base_url),
			Self::BASE_PATH,
			slug_url
		)
	}

	/// Build constants URL
	pub fn constants(base_url: &str) -> String {
		format!(
			"{}{}/constants",
			Self::normalize_base(base_url),
			Self::BASE_PATH
		)
	}

	/// Build auth/me URL for user information
	pub fn auth_me(base_url: &str) -> String {
		format!(
			"{}{}/auth/me",
			Self::normalize_base(base_url),
			Self::BASE_PATH
		)
	}

	/// Create manga search URL with query parameters
	pub fn manga_search_with_params(base_url: &str, params: &[(&str, &str)]) -> String {
		let base = Self::manga_search(base_url);
		Self::append_query_params(base, params)
	}

	/// Create manga details URL with fields
	pub fn manga_details_with_fields(base_url: &str, slug: &str, fields: &[&str]) -> String {
		let base = Self::manga_details(base_url, slug);
		let params: Vec<(&str, &str)> = fields.iter().map(|field| ("fields[]", *field)).collect();
		Self::append_query_params(base, &params)
	}

	/// Create chapter pages URL with parameters
	pub fn chapter_pages_with_params(
		base_url: &str,
		slug: &str,
		branch_id: Option<i32>,
		number: f32,
		volume: f32,
	) -> String {
		let base = Self::chapter_pages(base_url, slug);
		let mut params = vec![
			("number", number.to_string()),
			("volume", volume.to_string()),
		];

		if let Some(branch) = branch_id {
			params.push(("branch_id", branch.to_string()));
		}

		let param_refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
		Self::append_query_params(base, &param_refs)
	}

	/// Create constants URL with fields
	pub fn constants_with_fields(base_url: &str, fields: &[&str]) -> String {
		let base = Self::constants(base_url);
		let params: Vec<(&str, &str)> = fields.iter().map(|field| ("fields[]", *field)).collect();
		Self::append_query_params(base, &params)
	}

	/// Create top-views URL with all parameters
	pub fn top_views_with_params(base_url: &str, params: &[(&str, &str)]) -> String {
		let base = format!(
			"{}{}/media/top-views",
			Self::normalize_base(base_url),
			Self::BASE_PATH,
		);
		Self::append_query_params(base, params)
	}

	/// Normalize base URL by removing trailing slash
	fn normalize_base(base_url: &str) -> &str {
		base_url.trim_end_matches('/')
	}

	/// Append query parameters to URL
	fn append_query_params(base_url: String, params: &[(&str, &str)]) -> String {
		if params.is_empty() {
			return base_url;
		}

		let query_string = params
			.iter()
			.map(|(key, value)| format!("{key}={value}"))
			.collect::<Vec<_>>()
			.join("&");

		format!("{base_url}?{query_string}")
	}
}

#[cfg(test)]
mod test;
