use crate::crypto;
use aidoku::{
	FilterValue, Result,
	alloc::{String, string::ToString},
	error,
	helpers::uri::encode_uri,
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		net::{HttpMethod, Request},
		std::current_date,
	},
	prelude::*,
};
use core::fmt::{Display, Formatter, Result as FmtResult};
use md5::compute;

const API_URL: &str = "https://picaapi.picacomic.com";
const API_KEY: &str = "C69BAF41DA5ABD1FFEDC6D2FEA56B";
const KEY: &[u8; 63] = br"~d}$Q7$eIni=V)9\RK/P.RM4;9[7|@/CA}b~OW!3?EV`:<>M7pddUBL5n|0/*Cn";

pub fn gen_time() -> String {
	current_date().to_string()
}

pub fn gen_nonce() -> String {
	format!("{:x}", compute(gen_time()))
}

pub fn gen_signature(url: &str, time: &str, nonce: &str, method: &str) -> Result<String> {
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
		sort: String,
		page: i32,
	},
	Author {
		author: String,
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
		let mut category = String::new();
		let mut sort = String::from("dd");

		if let Some(q) = query {
			return Ok(Self::Search {
				query: q.to_string(),
				sort,
				page,
			});
		}

		for filter in filters {
			match filter {
				FilterValue::Text { id, value } => match id.as_str() {
					"author" => {
						return Ok(Self::Author {
							author: value.to_string(),
							sort,
							page,
						});
					}
					_ => {
						// Title search
						return Ok(Self::Search {
							query: value.to_string(),
							sort,
							page,
						});
					}
				},
				FilterValue::Select { id, value } => match id.as_str() {
					"类别" => {
						category = if value.as_str() == "全部" {
							String::new()
						} else {
							value.to_string()
						};
					}
					"genre" => {
						return Ok(Self::Search {
							query: value.to_string(),
							sort,
							page,
						});
					}
					_ => {}
				},
				FilterValue::Sort { id, index, .. } => {
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
			Url::Search { .. } => HttpMethod::Post,
			_ => HttpMethod::Get,
		};
		let body = match self {
			Url::Search { query, sort, .. } => Some(format!(
				r#"{{
					"keyword": "{}",
					"sort": "{}"
				}}"#,
				query, sort
			)),
			_ => None,
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
					gen_explore_url(category.to_string(), sort.to_string(), *page)
				)
			}
			Url::Search { page, .. } => {
				write!(f, "{}/comics/advanced-search?page={}", API_URL, page)
			}
			Url::Author { author, sort, page } => {
				write!(
					f,
					"{}/comics?page={}&a={}&s={}",
					API_URL,
					page,
					encode_uri(author),
					sort
				)
			}
			Url::Rank { time } => {
				write!(f, "{}", gen_rank_url(time.to_string()))
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
		bail!("Need to log in first");
	}

	let body = format!(
		r#"{{
			"email": "{}",
			"password": "{}"
		}}"#,
		username, password
	);

	let request = create_request(gen_login_url(), HttpMethod::Post, Some(body))?;
	// Override authorization header for login
	let request = request.header("Authorization", "");

	let mut response = request.send()?;

	if response.status_code() != 200 {
		bail!("Login failed");
	}

	let json: serde_json::Value = response.get_json()?;
	let data = json.get("data").ok_or(error!("No data in response"))?;
	let token = data.get("token").ok_or(error!("No token in response"))?;
	let token_str = token.as_str().ok_or(error!("Token is not a string"))?;

	defaults_set("token", DefaultValue::String(token_str.to_string()));

	Ok(token_str.to_string())
}

pub fn gen_login_url() -> String {
	format!("{}/{}", API_URL, "auth/sign-in")
}

pub fn create_request(url: String, method: HttpMethod, body: Option<String>) -> Result<Request> {
	let mut token: Option<String> = defaults_get("token");

	if url.contains("sign-in") {
		token = None;
	} else if token.is_none() {
		token = Some(login()?);
	}

	let mut request = Request::new(url.clone(), method)?;
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
		&gen_signature(
			&url,
			&gen_time(),
			&gen_nonce(),
			match method {
				HttpMethod::Get => "GET",
				HttpMethod::Post => "POST",
				_ => "GET",
			},
		)?,
	);
	request = request.header("Accept", "application/vnd.picacomic.com.v1+json");
	if let Some(ref token) = token {
		request = request.header("Authorization", token);
	}
	request = request.header("Content-Type", "application/json; charset=UTF-8");
	request = request.header("User-Agent", "okhttp/3.8.1");

	if let Some(body) = body {
		request = request.body(body.as_bytes());
	}

	Ok(request)
}

pub fn request_json<T>(url: Url) -> Result<T>
where
	T: serde::de::DeserializeOwned,
{
	let request = url.request()?;
	let mut response = request.send()?;

	if response.status_code() == 401 {
		// Token expired, login again
		login()?;
		// Retry with new token
		let request = url.request()?;
		response = request.send()?;
	}

	response.get_json_owned()
}
