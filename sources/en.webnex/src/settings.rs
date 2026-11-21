use aidoku::{
	alloc::{String, Vec},
	helpers::uri::encode_uri_component,
	imports::defaults::defaults_get,
	prelude::*,
};

pub const CONTENT_RATINGS: [&str; 4] = ["safe", "suggestive", "erotica", "pornographic"];
pub const CONTENT_TYPES: [&str; 3] = ["manga", "manhwa", "manhua"];

pub fn home_sections() -> Option<Vec<String>> {
	defaults_get("homeSections")
}

#[derive(Clone)]
pub struct Catalogue {
	pub ratings: Vec<String>,
	pub types: Vec<String>,
	pub hidden_genres: Vec<String>,
}

impl Catalogue {
	pub fn load() -> Self {
		let list = |key, default: &[&str]| {
			defaults_get(key).unwrap_or_else(|| default.iter().map(|v| (*v).into()).collect())
		};
		Self {
			ratings: list("contentRatings", &CONTENT_RATINGS[..2]),
			types: list("contentTypes", &CONTENT_TYPES),
			hidden_genres: list("hiddenGenres", &[]),
		}
	}

	pub fn ceiling(&self) -> Option<&'static str> {
		CONTENT_RATINGS
			.into_iter()
			.rev()
			.find(|r| self.ratings.iter().any(|v| v == r))
	}

	pub fn cookie(&self, rating: &str) -> String {
		let hidden: Vec<_> = CONTENT_TYPES
			.into_iter()
			.filter(|t| !self.types.iter().any(|v| v == t))
			.collect();
		let genres = serde_json::to_string(&self.hidden_genres).unwrap_or_default();
		format!(
			"content-rating={rating}; hide-types={}; hide-genres={}",
			encode_uri_component(hidden.join(",")),
			encode_uri_component(genres)
		)
	}
}

pub struct Groups {
	pub hidden: Vec<String>,
	pub preferred: Vec<String>,
	pub one_per_chapter: bool,
}

impl Groups {
	pub fn load() -> Self {
		let names = |key| {
			defaults_get::<Vec<String>>(key)
				.unwrap_or_default()
				.iter()
				.flat_map(|v| v.split(','))
				.map(|v| v.trim().to_lowercase())
				.filter(|v| !v.is_empty())
				.collect()
		};
		Self {
			hidden: names("excludedScanlators"),
			preferred: names("preferredScanlators"),
			one_per_chapter: defaults_get::<String>("chapterList").as_deref() == Some("chapter"),
		}
	}
}
