use aidoku::{
	alloc::{String, Vec},
	helpers::string::StripPrefixOrSelf,
	prelude::*,
	Manga,
};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchResponse {
	pub list: Vec<LilianaManga>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PageListResponse {
	pub status: bool,
	pub msg: Option<String>,
	pub html: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LilianaManga {
	pub cover: Option<String>,
	pub name: String,
	pub url: String,
}

impl LilianaManga {
	pub fn into_manga(self, base_url: &str) -> Manga {
		Manga {
			key: self.url.strip_prefix_or_self(base_url).into(),
			title: self.name,
			cover: self.cover.map(|cover| format!("{base_url}{cover}")),
			..Default::default()
		}
	}
}
