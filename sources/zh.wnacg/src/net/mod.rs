use crate::{BASE_URL, USER_AGENT};
use aidoku::{
	FilterValue, Result,
	alloc::{String, string::ToString as _},
	helpers::uri::encode_uri,
	imports::net::Request,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Clone)]
pub enum Url {
	Filter {
		category: String,
		tag: String,
		page: i32,
	},
	Search {
		query: String,
		page: i32,
	},
	Manga {
		id: String,
	},
	Chapter {
		id: String,
	},
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		let url = self.to_string();
		Ok(Request::get(url)?.header("User-Agent", USER_AGENT))
	}

	pub fn from_query_or_filters(
		query: Option<&str>,
		page: i32,
		filters: &[FilterValue],
	) -> Result<Self> {
		if let Some(q) = query {
			return Ok(Self::Search {
				query: encode_uri(q),
				page,
			});
		}

		let mut category = String::new();
		let mut tag = String::new();

		for filter in filters {
			match filter {
				FilterValue::Text { value, .. } => {
					return Ok(Self::Search {
						query: encode_uri(value.clone()),
						page,
					});
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"同人志" => category = value.clone(),
					"单行本" => category = value.clone(),
					"杂志&短篇" => category = value.clone(),
					"韩漫" => category = value.clone(),
					"genre" => tag = encode_uri(value.clone()),
					_ => {}
				},
				_ => {}
			}
		}

		Ok(Self::Filter {
			category,
			tag,
			page,
		})
	}

	pub fn manga(id: String) -> Self {
		Self::Manga { id }
	}

	pub fn chapter(id: String) -> Self {
		Self::Chapter { id }
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Search { query, page } => {
				write!(
					f,
					"{}/search/index.php?q={}&s=create_time_DESC&syn=yes&p={}",
					BASE_URL, query, page
				)
			}
			Url::Filter {
				category,
				tag,
				page,
			} => {
				if tag.is_empty() {
					write!(
						f,
						"{}/albums-index-page-{}-cate-{}.html",
						BASE_URL, page, category
					)
				} else {
					write!(
						f,
						"{}/albums-index-page-{}-tag-{}.html",
						BASE_URL, page, tag
					)
				}
			}
			Url::Manga { id } => {
				write!(f, "{}/photos-index-aid-{}.html", BASE_URL, id)
			}
			Url::Chapter { id } => {
				write!(f, "{}/photos-gallery-aid-{}.html", BASE_URL, id)
			}
		}
	}
}
