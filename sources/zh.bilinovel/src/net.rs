use crate::BASE_URL;
use crate::USER_AGENT;
use aidoku::{
	FilterValue, Result,
	alloc::{String, Vec, string::ToString as _},
	helpers::uri::encode_uri,
	imports::net::Request,
	prelude::*,
};
use core::fmt::{Display, Formatter, Result as FmtResult};
use regex::Regex;

#[derive(Clone)]
pub enum Url {
	Filter {
		order: &'static str,
		tagid: String,
		isfull: String,
		anime: String,
		rgroupid: String,
		update: String,
		words: String,
		page: i32,
	},
	Search {
		query: String,
		page: i32,
	},
	/// Ranking listings, e.g. `/top/monthvisit/1.html`.
	Top {
		order: &'static str,
		page: i32,
	},
	/// Completed-work rankings, e.g. `/topfull/weekvote/1.html`.
	TopFull {
		order: &'static str,
		page: i32,
	},
	/// The site home page, for the `Home` layout.
	Home,
	Manga {
		id: String,
	},
	/// An author's works, `/authorarticle/{author}.html`.
	Author {
		author: String,
	},
	/// A volume's page, `/novel/{id}/vol_{volume}.html`, which lists a
	/// volume's chapters with real links (the catalogue has placeholder
	/// `javascript:cid(N)` entries for some).
	Volume {
		manga_id: String,
		volume: String,
	},
	Catalog {
		id: String,
	},
	Chapter {
		manga_id: String,
		chapter_key: String,
		page: Option<i32>,
	},
	/// The shuffle-parameter script included by chapter pages.
	ChapterLog {
		src: String,
	},
}

impl Url {
	pub fn request(&self) -> Result<Request> {
		let url = self.to_string();
		let mut request = Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header("Origin", BASE_URL)
			.header("Accept-Language", "zh-CN,zh;q=0.9")
			.header("Cookie", "night=1");

		// Search requires a jieqi search ticket (mirrors mihon's ensureSearchTicket)
		// plus a referer pointing at the search form.
		if let Url::Search { query, .. } = self {
			let ticket = ensure_search_ticket();
			let referer = format!("{}/search.html?searchkey={}", BASE_URL, query);
			request = request
				.header("Referer", &referer)
				.header("Cookie", &ticket);
		}

		Ok(request)
	}

	pub fn from_query_or_filters(
		query: Option<&str>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<Self> {
		if let Some(q) = query {
			return Ok(Self::Search {
				query: encode_uri(q),
				page,
			});
		}

		let mut order: &'static str = "lastupdate";
		let mut tagid = String::from("0");
		let mut isfull = String::from("0");
		let mut anime = String::from("0");
		let mut rgroupid = String::from("0");
		let mut update = String::from("0");
		let mut words = String::from("0");

		for filter in filters {
			match filter {
				// Author search (synthesized by the app via
				// `supportsAuthorSearch`): the site has a dedicated page
				// listing the author's works.
				FilterValue::Text { id, value } if id.as_str() == "author" => {
					return Ok(Self::Author {
						author: encode_uri(&value),
					});
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"连载状态" => isfull = value,
					"动画化" => anime = value,
					"文库地区" => rgroupid = value,
					"更新时间" => update = value,
					"字数" => words = value,
					// A tag tapped on a book (synthesized by the app via
					// `supportsTagSearch`): the value is the tag name.
					"genre" => {
						if let Some(tag) = crate::tags::tag_id(&value) {
							tagid = tag.to_string();
						}
					}
					_ => {}
				},
				FilterValue::MultiSelect { id, included, .. }
					if id.as_str() == "作品主题" && !included.is_empty() =>
				{
					// The site supports up to 4 comma-joined theme ids.
					tagid = included.join("-");
				}
				FilterValue::Sort { id, index, .. } if id.as_str() == "排序" => {
					let orders = [
						"lastupdate",
						"postdate",
						"weekvisit",
						"monthvisit",
						"weekvote",
						"monthvote",
						"weekflower",
						"monthflower",
						"words",
						"goodnum",
					];
					if let Some(o) = orders.get(index as usize) {
						order = o;
					}
				}
				_ => {}
			}
		}

		Ok(Self::Filter {
			order,
			tagid,
			isfull,
			anime,
			rgroupid,
			update,
			words,
			page,
		})
	}

	pub fn manga(id: &str) -> Self {
		Self::Manga { id: id.to_string() }
	}

	/// A `/wenku/{order}_..._{page}_0.html` listing with every filter
	/// defaulted.
	pub fn wenku(order: &'static str, page: i32) -> Self {
		Self::Filter {
			order,
			tagid: "0".to_string(),
			isfull: "0".to_string(),
			anime: "0".to_string(),
			rgroupid: "0".to_string(),
			update: "0".to_string(),
			words: "0".to_string(),
			page,
		}
	}

	/// A `/top/{order}/{page}.html` ranking listing.
	pub fn top(order: &'static str, page: i32) -> Self {
		Self::Top { order, page }
	}

	/// A `/topfull/{order}/{page}.html` completed-work ranking.
	pub fn topfull(order: &'static str, page: i32) -> Self {
		Self::TopFull { order, page }
	}

	/// The site home page.
	pub fn home() -> Self {
		Self::Home
	}

	pub fn catalog(id: &str) -> Self {
		Self::Catalog { id: id.to_string() }
	}

	/// A volume's page, `/novel/{id}/vol_{volume}.html`.
	pub fn volume(manga_id: &str, volume: &str) -> Self {
		Self::Volume {
			manga_id: manga_id.to_string(),
			volume: volume.to_string(),
		}
	}

	pub fn chapter(manga_id: &str, chapter_key: &str) -> Self {
		Self::Chapter {
			manga_id: manga_id.to_string(),
			chapter_key: chapter_key.to_string(),
			page: None,
		}
	}

	pub fn chapter_page(manga_id: &str, chapter_key: &str, page: i32) -> Self {
		Self::Chapter {
			manga_id: manga_id.to_string(),
			chapter_key: chapter_key.to_string(),
			page: Some(page),
		}
	}
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Filter {
				order,
				tagid,
				isfull,
				anime,
				rgroupid,
				update,
				words,
				page,
			} => {
				write!(
					f,
					"{}/wenku/{}_{}_{}_{}_{}_{}_0_{}_{}_0.html",
					BASE_URL, order, tagid, isfull, anime, rgroupid, update, words, page
				)
			}
			Url::Search { query, page } => {
				write!(f, "{}/search/{}_{}.html", BASE_URL, query, page)
			}
			Url::Top { order, page } => {
				write!(f, "{}/top/{}/{}.html", BASE_URL, order, page)
			}
			Url::TopFull { order, page } => {
				write!(f, "{}/topfull/{}/{}.html", BASE_URL, order, page)
			}
			Url::Home => {
				write!(f, "{}/", BASE_URL)
			}
			Url::Manga { id } => {
				write!(f, "{}/novel/{}.html", BASE_URL, id)
			}
			Url::Author { author } => {
				write!(f, "{}/authorarticle/{}.html", BASE_URL, author)
			}
			Url::Volume { manga_id, volume } => {
				write!(f, "{}/novel/{}/vol_{}.html", BASE_URL, manga_id, volume)
			}
			Url::Catalog { id } => {
				write!(f, "{}/novel/{}/catalog", BASE_URL, id)
			}
			Url::Chapter {
				manga_id,
				chapter_key,
				page,
			} => match page {
				Some(p) => write!(
					f,
					"{}/novel/{}/{}_{}.html",
					BASE_URL, manga_id, chapter_key, p
				),
				None => write!(f, "{}/novel/{}/{}.html", BASE_URL, manga_id, chapter_key),
			},
			Url::ChapterLog { src } => {
				if src.starts_with("http") {
					write!(f, "{}", src)
				} else {
					write!(f, "{}{}", BASE_URL, src)
				}
			}
		}
	}
}

/// Append `name=value` pairs parsed from a `Set-Cookie` (or inline
/// `document.cookie="..."`) string into the running cookie header, skipping
/// the `night` flag which is already managed separately.
fn append_cookies(cookie: &mut String, set_cookie: &str) {
	for part in set_cookie.split(',') {
		let nv = part.split(';').next().unwrap_or("").trim();
		if nv.is_empty() || !nv.contains('=') {
			continue;
		}
		let name = match nv.split('=').next() {
			Some(n) => n,
			None => continue,
		};
		if name == "night" {
			continue;
		}
		if !cookie.contains(&format!("{}=", name)) {
			cookie.push_str("; ");
			cookie.push_str(nv);
		}
	}
}

fn run_guard(path: &str, cookie: &mut String) {
	let req = match Request::get(format!("{}{}", BASE_URL, path)) {
		Ok(r) => r,
		Err(_) => return,
	}
	.header("User-Agent", USER_AGENT)
	.header("Origin", BASE_URL)
	.header("Accept-Language", "zh-CN,zh;q=0.9")
	.header("Cookie", cookie.as_str());
	if let Ok(resp) = req.send() {
		if let Some(sc) = resp.get_header("Set-Cookie") {
			append_cookies(cookie, &sc);
		}
		if path.contains("search_guard=js")
			&& let Ok(body) = resp.get_string()
			&& let Ok(re) = Regex::new("cookie=\"(.*?)\";")
			&& let Some(c) = re.captures(&body).and_then(|c| c.get(1))
		{
			append_cookies(cookie, c.as_str());
		}
	}
}

/// Mirror of mihon's `ensureSearchTicket`: visit the search guard pages to
/// obtain the `jieqiSearchTicket` cookies, returning a ready-to-send
/// `Cookie` header value (always starts with `night=1`).
pub fn ensure_search_ticket() -> String {
	let mut cookie = String::from("night=1");
	run_guard("/search.html?search_guard=css", &mut cookie);
	run_guard("/search.html?search_guard=js", &mut cookie);
	run_guard("/search.html?search_guard=redeem", &mut cookie);
	cookie
}
