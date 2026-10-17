use crate::BASE_URL;
use aidoku::{
	FilterValue, Result,
	alloc::{String, string::ToString as _},
	helpers::uri::encode_uri,
	imports::{html::Document, net::Request, std::sleep},
	prelude::*,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Clone)]
pub enum Url {
	Search {
		query: String,
		page: i32,
	},
	Author {
		name: String,
		page: i32,
	},
	Filter {
		path: String,
		genre: String,
		page: i32,
	},
}

impl Url {
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

		let mut path = String::new();
		let mut genre = String::new();

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => {
					if id == "author" && !value.is_empty() {
						let slug = value.to_lowercase().replace(' ', "-");
						return Ok(Self::Author { name: slug, page });
					}
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"分类" => path = value.to_string(),
					"genre" => genre = value.to_lowercase().replace('/', "-"),
					_ => {}
				},
				_ => continue,
			}
		}

		Ok(Self::Filter { path, genre, page })
	}

	pub fn request(&self) -> Result<Request> {
		let url = self.to_string();
		Ok(Request::get(url)?)
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Search { query, page } => {
				write!(
					f,
					"{}/page/{}/?s={}&post_type=wp-manga",
					BASE_URL, page, query
				)
			}
			Url::Author { name, page } => {
				write!(f, "{}/manga-author/{}/page/{}/", BASE_URL, name, page)
			}
			Url::Filter { path, genre, page } => {
				if !genre.is_empty() {
					write!(f, "{}/manga-tag/{}/page/{}/", BASE_URL, genre, page)
				} else if !path.is_empty() {
					write!(f, "{}/{}/page/{}/", BASE_URL, path, page)
				} else {
					write!(f, "{}/page/{}/", BASE_URL, page)
				}
			}
		}
	}
}

const CHALLENGE_TRIES: u32 = 4;
const CHALLENGE_PAUSE: i32 = 3;

fn is_challenge(html: &Document) -> bool {
	html.select_first("#challenge-error-title, #challenge-error-text")
		.is_some()
}

/// Sends one request. `None` when the app's Cloudflare handling still hands back
/// the challenge page.
fn fetch_page(request: Request) -> Result<Option<Document>> {
	let response = request.send()?;
	let html = response.get_html()?;
	if is_challenge(&html) {
		println!(
			"[bakamh] Cloudflare page: status={} cf-ray={:?}",
			response.status_code(),
			response.get_header("cf-ray"),
		);
		return Ok(None);
	}
	Ok(Some(html))
}

/// Fetches a page, erroring on a Cloudflare challenge without retrying.
pub fn fetch_html(request: Request) -> Result<Document> {
	match fetch_page(request)? {
		Some(html) => Ok(html),
		None => bail!("[bakamh] Cloudflare still blocks the request"),
	}
}

/// Fetches a page, retrying on a Cloudflare challenge with a pause before each new
/// try. Any other failure, such as being offline, is not retried.
pub fn fetch_html_retrying(mut request: impl FnMut() -> Result<Request>) -> Result<Document> {
	for tries in 1..=CHALLENGE_TRIES {
		if let Some(html) = fetch_page(request()?)? {
			return Ok(html);
		}
		if tries < CHALLENGE_TRIES {
			println!(
				"[bakamh] Cloudflare still blocks after try {tries} of {CHALLENGE_TRIES}, trying again in {CHALLENGE_PAUSE} s"
			);
			sleep(CHALLENGE_PAUSE);
		}
	}
	bail!("[bakamh] Cloudflare still blocks after {CHALLENGE_TRIES} tries")
}
