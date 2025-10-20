use crate::crypto;
use aidoku::{
	AidokuError, FilterValue, Result,
	alloc::{String, string::ToString},
	helpers::uri::encode_uri,
	imports::{
		defaults::{defaults_get, defaults_set},
		net::{HttpMethod, Request},
	},
	prelude::*,
};
use core::fmt::{Display, Formatter, Result as FmtResult};
use md5::compute;

const API_URL: &str = "https://picaapi.picacomic.com";
const API_KEY: &str = "C69BAF41DA5ABD1FFEDC6D2FEA56B";
const KEY: &[u8; 63] = br"~d}$Q7$eIni=V)9\RK/P.RM4;9[7|@/CA}b~OW!3?EV`:<>M7pddUBL5n|0/*Cn";

pub fn gen_time() -> String {
	aidoku::imports::std::current_date().to_string()
}

pub fn gen_nonce() -> String {
	format!("{:x}", compute(gen_time()))
}

pub fn gen_signature(url: &str, time: &str, nonce: &str, method: &str) -> String {
	let url = url.trim_start_matches(&format!("{}/", API_URL));
	let text = format!("{}{}{}{}{}", url, time, nonce, method, API_KEY).to_ascii_lowercase();
	crypto::encrypt(text.as_bytes(), KEY)
}

#[derive(Clone)]
pub enum Url {
	Explore {
		category: String,
		sort: String,
		page: i32,
	},
	Search {
		query: String,
		page: i32,
	},
	Author {
		author: String,
		sort: String,
		page: i32,
	},
	Tag {
		tag: String,
		sort: String,
		page: i32,
	},
	Rank {
		time: String,
	},
	Random,
	Favourite {
		sort: String,
		page: i32,
	},
	Manga {
		id: String,
	},
	ChapterList {
		id: String,
		page: i32,
	},
	PageList {
		manga_id: String,
		chapter_id: String,
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
				query: q.to_string(),
				page,
			});
		}

		let mut category = String::new();
		let mut sort = String::from("dd");

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" => {
						return Ok(Self::Author {
							author: value.to_string(),
							sort: sort.clone(),
							page,
						});
					}
					_ => {
						// Title search
						return Ok(Self::Search {
							query: value.to_string(),
							page,
						});
					}
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"类别" => {
						category = if value.as_str() == "全部" {
							String::new()
						} else {
							value.to_string()
						};
					}
					"genre" => {
						return Ok(Self::Tag {
							tag: value.to_string(),
							sort: sort.clone(),
							page,
						});
					}
					_ => {}
				},
				FilterValue::Sort {
					id,
					index,
					ascending: _,
				} => {
					if id.as_str() == "排序" {
						let sorts = ["dd", "da", "ld", "vd"];
						if let Some(s) = sorts.get(*index as usize) {
							sort = s.to_string();
						}
					}
				}
				_ => {}
			}
		}

		Ok(Self::Explore {
			category,
			sort,
			page,
		})
	}

	pub fn request(&self) -> Result<Request> {
		let url = self.to_string();
		let method = match self {
			Url::Search { .. } => "POST",
			_ => "GET",
		};
		let body = match self {
			Url::Search { query, .. } => {
				format!(
					r#"{{
						"keyword": "{}",
						"sort": "dd"
					}}"#,
					query
				)
			}
			_ => String::new(),
		};

		create_request(url, method, body)
	}
}

pub fn gen_explore_url(category: String, sort: String, page: i32) -> String {
	if category.is_empty() {
		format!("{}/comics?page={}&s={}", API_URL, page, sort)
	} else {
		format!(
			"{}/comics?page={}&c={}&s={}",
			API_URL,
			page,
			encode_uri(category),
			sort
		)
	}
}

pub fn gen_rank_url(time: String) -> String {
	format!("{}/comics/leaderboard?tt={}&ct=VC", API_URL, time)
}

impl Display for Url {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Url::Explore {
				category,
				sort,
				page,
			} => {
				write!(
					f,
					"{}",
					gen_explore_url(category.clone(), sort.clone(), *page)
				)
			}
			Url::Search { query: _, page } => {
				write!(f, "{}/comics/advanced-search?page={}&s=dd", API_URL, page)
			}
			Url::Author { author, sort, page } => {
				write!(
					f,
					"{}/comics?page={}&a={}&s={}",
					API_URL,
					page,
					encode_uri(author.clone()),
					sort
				)
			}
			Url::Tag { tag, sort, page } => {
				write!(
					f,
					"{}/comics?page={}&c={}&s={}",
					API_URL,
					page,
					encode_uri(tag.clone()),
					sort
				)
			}
			Url::Rank { time } => {
				write!(f, "{}", gen_rank_url(time.clone()))
			}
			Url::Random => {
				write!(f, "{}/comics/random", API_URL)
			}
			Url::Favourite { sort, page } => {
				write!(f, "{}/users/favourite?page={}&s={}", API_URL, page, sort)
			}
			Url::Manga { id } => {
				write!(f, "{}/comics/{}", API_URL, id)
			}
			Url::ChapterList { id, page } => {
				write!(f, "{}/comics/{}/eps?page={}", API_URL, id, page)
			}
			Url::PageList {
				manga_id,
				chapter_id,
				page,
			} => {
				write!(
					f,
					"{}/comics/{}/order/{}/pages?page={}",
					API_URL, manga_id, chapter_id, page
				)
			}
		}
	}
}

pub fn login() -> Result<String> {
	let username = crate::settings::get_username()?;
	let password = crate::settings::get_password()?;

	if username.is_empty() || password.is_empty() {
		return Err(AidokuError::message("Username or password not set"));
	}

	let body = format!(
		r#"{{
			"email": "{}",
			"password": "{}"
		}}"#,
		username, password
	);

	let request = create_request(gen_login_url(), "POST", body)?;
	// Override authorization header for login
	let request = request.header("Authorization", "");

	let mut response = request.send()?;

	if response.status_code() != 200 {
		return Err(AidokuError::message("Login failed"));
	}

	let json: serde_json::Value = response.get_json()?;
	let data = json
		.get("data")
		.ok_or(AidokuError::message("No data in response"))?;
	let token = data
		.get("token")
		.ok_or(AidokuError::message("No token in response"))?;
	let token_str = token
		.as_str()
		.ok_or(AidokuError::message("Token is not a string"))?;

	defaults_set(
		"token",
		aidoku::imports::defaults::DefaultValue::String(token_str.to_string()),
	);

	Ok(token_str.to_string())
}

pub fn gen_login_url() -> String {
	format!("{}/{}", API_URL, "auth/sign-in")
}

pub fn create_request(url: String, method: &str, body: String) -> Result<Request> {
	let mut token = defaults_get::<String>("token").unwrap_or_default();

	if url.contains("sign-in") {
		token = String::new();
	} else if token.is_empty() {
		token = login()?;
	}

	let http_method = match method {
		"POST" => HttpMethod::Post,
		_ => HttpMethod::Get,
	};

	let mut request = Request::new(url.clone(), http_method)?;
	request = request.header("api-key", API_KEY);
	request = request.header("app-build-version", "45");
	request = request.header("app-channel", &crate::settings::get_app_channel());
	request = request.header("app-platform", "android");
	request = request.header("app-uuid", "defaultUuid");
	request = request.header("app-version", "2.2.1.3.3.4");
	request = request.header("image-quality", &crate::settings::get_image_quality());
	request = request.header("time", &gen_time());
	request = request.header("nonce", &gen_nonce());
	request = request.header(
		"signature",
		&gen_signature(&url, &gen_time(), &gen_nonce(), method),
	);
	request = request.header("Accept", "application/vnd.picacomic.com.v1+json");
	if !token.is_empty() {
		request = request.header("Authorization", &token);
	}
	request = request.header("Content-Type", "application/json; charset=UTF-8");
	request = request.header("User-Agent", "okhttp/3.8.1");

	if !body.is_empty() {
		request = request.body(body.as_bytes());
	}

	Ok(request)
}
