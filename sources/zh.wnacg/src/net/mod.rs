use crate::USER_AGENT;
use aidoku::{
	FilterValue, Result,
	alloc::{String, Vec, format},
	error,
	helpers::uri::encode_uri,
	imports::{defaults::defaults_get, net::Request},
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
	/// All chapters of a series in one JSON response; the site's own reader
	/// populates its chapter menu from it. Returns 404 when logged out.
	SeriesChapters {
		sid: String,
	},
}

pub fn base_url() -> Result<String> {
	defaults_get::<String>("url")
		.ok_or_else(|| error!("Default not exist or not string for key: `url`"))
}

/// Request with the login cookie attached when logged in: the korean
/// category, its galleries and their reader pages all 404 without it.
pub fn request(url: &str) -> Result<Request> {
	request_with_cookie(url, crate::auth::cookie_header().as_deref())
}

/// Explicit-cookie variant of `request`, so login verification can try a
/// header before it is stored.
pub fn request_with_cookie(url: &str, cookie: Option<&str>) -> Result<Request> {
	let request = Request::get(url)?.header("User-Agent", USER_AGENT);
	Ok(match cookie {
		Some(cookie) => request.header("Cookie", cookie),
		None => request,
	})
}

/// `/photos-index-aid-385618.html` → `385618`.
pub fn aid_from_href(href: &str) -> Option<String> {
	digits_after(href, "-aid-")
}

/// `/photos-slide-aid-117508-sid-387771.html` → `387771`, the series the
/// chapter belongs to.
pub fn sid_from_href(href: &str) -> Option<String> {
	digits_after(href, "-sid-")
}

fn digits_after(href: &str, marker: &str) -> Option<String> {
	let index = href.find(marker)?;
	let digits: String = href[index + marker.len()..]
		.chars()
		.take_while(char::is_ascii_digit)
		.collect();
	(!digits.is_empty()).then_some(digits)
}

impl Url {
	pub fn to_string(&self) -> Result<String> {
		Ok(format!("{}{self}", base_url()?))
	}

	pub fn request(&self) -> Result<Request> {
		let url = self.to_string()?;
		request(&url)
	}

	pub fn from_query_or_filters(
		query: Option<&str>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<Self> {
		let mut category = String::new();
		let mut tag = String::new();

		for filter in filters {
			match filter {
				FilterValue::Text { value, .. } => {
					// There is no user-configurable text filter, so a text value
					// can only come from a tap-to-search on a gallery page
					// (supportsAuthorSearch): the author from the title's
					// bracket prefix, or the artist circle inside the same
					// prefix (`[社团 (画师)]`). The site search matches both.
					return Ok(Self::Search {
						query: encode_uri(value),
						page,
					});
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"同人志" => category = value,
					"单行本" => category = value,
					"杂志&短篇" => category = value,
					"韩漫" => category = value,
					// Tag tap on a gallery page (supportsTagSearch).
					"genre" => tag = encode_uri(value),
					_ => {}
				},
				_ => {}
			}
		}

		// Keyword search, tag browsing and category browsing are three disjoint
		// endpoints; the most specific input wins.
		if let Some(query) = query {
			return Ok(Self::Search {
				query: encode_uri(query),
				page,
			});
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

	pub fn series_chapters(sid: String) -> Self {
		Self::SeriesChapters { sid }
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Search { query, page } => {
				write!(
					f,
					"/search/index.php?q={}&s=create_time_DESC&syn=yes&p={}",
					query, page
				)
			}
			Url::Filter {
				category,
				tag,
				page,
			} => {
				if tag.is_empty() {
					write!(f, "/albums-index-page-{}-cate-{}.html", page, category)
				} else {
					write!(f, "/albums-index-page-{}-tag-{}.html", page, tag)
				}
			}
			Url::Manga { id } => {
				write!(f, "/photos-index-aid-{}.html", id)
			}
			Url::Chapter { id } => {
				write!(f, "/photos-gallery-aid-{}.html", id)
			}
			Url::SeriesChapters { sid } => {
				write!(f, "/?ctl=photos&act=chapters&sid={}", sid)
			}
		}
	}
}
