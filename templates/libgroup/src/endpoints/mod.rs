use aidoku::{
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
};
use strum::Display;

#[derive(Display, Debug, Clone)]
pub enum Url<'a> {
	#[strum(serialize = "/api/manga")]
	MangaSearch,

	#[strum(serialize = "/api/manga/{slug}")]
	MangaDetails { slug: &'a str },

	#[strum(serialize = "/api/manga/{slug}/chapters")]
	MangaChapters { slug: &'a str },

	#[strum(serialize = "/api/manga/{slug}/chapter")]
	ChapterPages {
		slug: &'a str,
		branch_id: Option<u32>,
		number: f32,
		volume: f32,
	},

	#[strum(serialize = "/api/constants")]
	Constants,
}

impl<'a> Url<'a> {
	/// Build full URL with base URL from defaults_get
	pub fn build(&self, base_url: &str) -> String {
		let base = base_url.trim_end_matches('/');

		match self {
			Self::MangaSearch => format!("{base}/api/manga"),
			Self::MangaDetails { slug } => format!("{base}/api/manga/{slug}"),
			Self::MangaChapters { slug } => format!("{base}/api/manga/{slug}/chapters"),
			Self::ChapterPages { slug, .. } => format!("{base}/api/manga/{slug}/chapter"),
			Self::Constants => format!("{base}/api/constants"),
		}
	}

	/// Create manga search URL with query parameters
	pub fn manga_search_with_params(base_url: &str, params: &[(&str, &str)]) -> String {
		let base = Self::MangaSearch.build(base_url);
		if params.is_empty() {
			return base;
		}

		let query_string = params
			.iter()
			.map(|(key, value)| format!("{key}={value}"))
			.collect::<Vec<_>>()
			.join("&");

		format!("{base}?{query_string}")
	}

	/// Create manga details URL with fields
	pub fn manga_details_with_fields(base_url: &str, slug: &'a str, fields: &[&str]) -> String {
		let base = Self::MangaDetails { slug }.build(base_url);
		if fields.is_empty() {
			return base;
		}

		let query_string = fields
			.iter()
			.map(|field| format!("fields[]={field}"))
			.collect::<Vec<_>>()
			.join("&");

		format!("{base}?{query_string}")
	}

	/// Create chapter pages URL with parameters
	pub fn chapter_pages_with_params(
		base_url: &str,
		slug: &'a str,
		branch_id: Option<u32>,
		number: f32,
		volume: f32,
	) -> String {
		let base = Self::ChapterPages {
			slug,
			branch_id,
			number,
			volume,
		}
		.build(base_url);
		let mut params = vec![format!("number={}", number), format!("volume={}", volume)];

		if let Some(branch) = branch_id {
			params.push(format!("branch_id={branch}"));
		}

		format!("{}?{}", base, params.join("&"))
	}

	/// Create image servers URL with fields
	pub fn constants_with_fields(base_url: &str, fields: &[&str]) -> String {
		let base = Self::Constants.build(base_url);
		if fields.is_empty() {
			return base;
		}

		let query_string = fields
			.iter()
			.map(|field| format!("fields[]={field}"))
			.collect::<Vec<_>>()
			.join("&");

		format!("{base}?{query_string}")
	}
}

impl From<Url<'_>> for String {
	fn from(url: Url<'_>) -> Self {
		url.to_string()
	}
}

#[cfg(test)]
mod test;
