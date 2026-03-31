use aes::{
	Aes256,
	cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray},
};
use aidoku::{
	Result,
	alloc::{String, Vec, format},
	error,
	imports::net::{Request, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::de::DeserializeOwned;

use crate::models::{ApiOuter, AuthData, DomainRefreshResp, PromoteGroup, SearchResp, SettingData};
use crate::settings;

const DOMAIN_REFRESH_URLS: [&str; 2] = [
	"https://rup4a04-c01.tos-ap-southeast-1.bytepluses.com/newsvr-2025.txt",
	"https://rup4a04-c02.tos-cn-hongkong.bytepluses.com/newsvr-2025.txt",
];
const DOMAIN_REFRESH_SECRET: &str = "diosfjckwpqpdfjkvnqQjsik";

const JM_VERSION: &str = "2.0.16";
pub const JM_UA: &str = "Mozilla/5.0 (Linux; Android 10; K; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/130.0.0.0 Mobile Safari/537.36";
pub const JM_PKG: &str = "com.example.app";

const JM_AUTH_KEY: &str = "18comicAPPContent";
const JM_DATA_SECRET: &str = "185Hcomic3PAPP7R";

// image CDN server preference order
const PREFERRED_IMAGE_SHUNTS: [u8; 4] = [3, 2, 4, 1];

fn normalize_cdn_base(url: &str) -> Option<String> {
	let url = url.trim().trim_end_matches('/');
	(!url.is_empty() && url.starts_with("http")).then(|| url.into())
}

fn normalize_domain(domain: &str) -> Option<String> {
	let domain = domain
		.trim()
		.trim_start_matches("https://")
		.trim_start_matches("http://")
		.trim_end_matches('/');
	(!domain.is_empty()).then(|| domain.into())
}

pub struct ApiContext {
	domain: String,
	pub cdn_base: String,
}

impl ApiContext {
	pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
		api_get_on_domain(&self.domain, path)
	}
}

pub fn context() -> Result<ApiContext> {
	let ts = current_ts();
	for domain in fetch_domain_candidates() {
		for shunt in PREFERRED_IMAGE_SHUNTS {
			if let Ok(setting) = api_get_on_domain_with_auth::<SettingData>(
				&domain,
				&url::setting_path(shunt),
				ts,
				None,
			) && let Some(cdn_base) = setting.img_host.as_deref().and_then(normalize_cdn_base)
			{
				return Ok(ApiContext { domain, cdn_base });
			}
		}
	}
	Err(error!("当前所有域名都暂时不可用"))
}

pub mod url {
	use aidoku::{
		alloc::{String, format},
		helpers::uri::encode_uri_component,
	};

	pub fn album(id: &str) -> String {
		format!("/album?id={}", id)
	}

	pub fn chapter(id: &str) -> String {
		format!("/chapter?id={}", id)
	}

	pub fn search(query: &str, order: &str, category: &str, page: i32) -> String {
		let q = encode_uri_component(query);
		if category.is_empty() {
			format!("/search?search_query={}&o={}&page={}", q, order, page)
		} else {
			let c = encode_uri_component(category);
			format!(
				"/search?search_query={}&o={}&c={}&page={}",
				q, order, c, page
			)
		}
	}

	pub fn filter(order: &str, category: &str, page: i32) -> String {
		if category.is_empty() {
			format!("/categories/filter?o={}&page={}", order, page)
		} else {
			let c = encode_uri_component(category);
			format!("/categories/filter?o={}&c={}&page={}", order, c, page)
		}
	}

	pub fn promote(page: i32) -> String {
		format!("/promote?page={}", page)
	}

	pub fn setting_path(shunt: u8) -> String {
		format!("/setting?app_img_shunt={shunt}&express=")
	}

	pub fn login_path() -> &'static str {
		"/login"
	}
}

fn md5_hex(input: &[u8]) -> String {
	format!("{:x}", md5::compute(input))
}

pub fn current_ts() -> u64 {
	aidoku::imports::std::current_date() as u64
}

fn api_auth(ts: u64) -> (String, String) {
	(
		md5_hex(format!("{ts}{JM_AUTH_KEY}").as_bytes()),
		format!("{ts},{JM_VERSION}"),
	)
}

fn with_user_agent(request: Request) -> Request {
	request.header("user-agent", JM_UA)
}

fn with_api_headers(request: Request, token: &str, tokenparam: &str) -> Request {
	with_user_agent(request)
		.header("token", token)
		.header("tokenparam", tokenparam)
		.header("x-requested-with", JM_PKG)
		.header("accept", "*/*")
}

fn with_browser_headers(request: Request) -> Request {
	request
		.header("origin", "https://localhost")
		.header("referer", "https://localhost/")
}

fn derive_aes_key(secret: &str) -> [u8; 32] {
	let hex = md5_hex(secret.as_bytes());
	let mut key = [0u8; 32];
	key.copy_from_slice(hex.as_bytes());
	key
}

fn aes256_ecb_decrypt(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
	if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) {
		return Err(error!("响应数据长度异常：{}", ciphertext.len()));
	}
	let cipher = Aes256::new_from_slice(key).map_err(|_| error!("解密器初始化失败"))?;
	let mut data: Vec<u8> = ciphertext.into();
	for chunk in data.chunks_exact_mut(16) {
		cipher.decrypt_block(GenericArray::from_mut_slice(chunk));
	}
	let pad = *data.last().ok_or_else(|| error!("解密结果为空"))? as usize;
	if pad == 0 || pad > 16 || data.len() < pad {
		return Err(error!("响应填充数据异常：{}", pad));
	}
	data.truncate(data.len() - pad);
	Ok(data)
}

fn decrypt_data(data_b64: &str, ts: u64) -> Result<Vec<u8>> {
	let key = derive_aes_key(&format!("{ts}{JM_DATA_SECRET}"));
	let ct = STANDARD
		.decode(data_b64.as_bytes())
		.map_err(|_| error!("响应解码失败"))?;
	let raw = aes256_ecb_decrypt(&ct, &key)?;

	let s = core::str::from_utf8(&raw).map_err(|_| error!("响应文本解析失败"))?;
	if s.is_empty() {
		return Err(error!("响应文本为空"));
	}
	let start = s.find(['{', '[']).unwrap_or(0);
	let end = s.rfind(['}', ']']).unwrap_or(s.len().saturating_sub(1));
	let Some(json) = s.as_bytes().get(start..=end) else {
		return Err(error!("响应边界异常"));
	};
	Ok(json.into())
}

pub fn parse_response<T: DeserializeOwned>(resp: Response, ts: u64) -> Result<T> {
	let body = resp.get_string()?;
	let outer: ApiOuter = serde_json::from_str(&body).map_err(|_| error!("外层响应解析失败"))?;
	let decrypted = decrypt_data(&outer.data, ts)?;
	serde_json::from_slice(&decrypted).map_err(|_| error!("内容数据解析失败"))
}

fn api_get_on_domain<T: DeserializeOwned>(domain: &str, path: &str) -> Result<T> {
	let auth = settings::get_auth();
	let ts = current_ts();
	api_get_on_domain_with_auth(
		domain,
		path,
		ts,
		auth.as_ref().map(|auth| auth.jwt_token.as_str()),
	)
}

pub fn home_data(ctx: &ApiContext) -> Result<(Vec<PromoteGroup>, SearchResp)> {
	let auth = settings::get_auth();
	let ts = current_ts();
	let bearer = auth.as_ref().map(|a| a.jwt_token.as_str());

	let req1 = get_request_on_domain(&ctx.domain, &url::promote(0), ts, bearer)?;
	let req2 = get_request_on_domain(&ctx.domain, &url::filter("mr", "single", 1), ts, bearer)?;

	let mut responses = Request::send_all([req1, req2]).into_iter();
	let groups: Vec<PromoteGroup> = responses
		.next()
		.ok_or_else(|| error!("响应缺失"))?
		.map_err(|_| error!("请求失败"))
		.and_then(|r| parse_response(r, ts))?;
	let single: SearchResp = responses
		.next()
		.ok_or_else(|| error!("响应缺失"))?
		.map_err(|_| error!("请求失败"))
		.and_then(|r| parse_response(r, ts))?;

	Ok((groups, single))
}

fn fetch_domain_candidates() -> Vec<String> {
	let key = derive_aes_key(DOMAIN_REFRESH_SECRET);
	for url in DOMAIN_REFRESH_URLS {
		let Ok(resp) = Request::get(url)
			.map(with_user_agent)
			.and_then(|r| r.send())
		else {
			continue;
		};
		let Ok(ct_b64) = resp.get_string() else {
			continue;
		};
		let ct_b64 = ct_b64.trim().trim_start_matches('\u{feff}');
		let Ok(ct) = STANDARD.decode(ct_b64.as_bytes()) else {
			continue;
		};
		let Ok(raw) = aes256_ecb_decrypt(&ct, &key) else {
			continue;
		};
		if let Ok(parsed) = serde_json::from_slice::<DomainRefreshResp>(&raw)
			&& !parsed.server.is_empty()
		{
			let mut domains = Vec::new();
			for domain in parsed.server {
				if let Some(domain) = normalize_domain(&domain)
					&& !domains.iter().any(|current| current == &domain)
				{
					domains.push(domain);
				}
			}
			if !domains.is_empty() {
				return domains;
			}
		}
	}
	Vec::new()
}

fn api_get_on_domain_with_auth<T: DeserializeOwned>(
	domain: &str,
	path: &str,
	ts: u64,
	bearer_token: Option<&str>,
) -> Result<T> {
	let resp = get_request_on_domain(domain, path, ts, bearer_token)?.send()?;
	parse_response(resp, ts)
}

fn get_request_on_domain(
	domain: &str,
	path: &str,
	ts: u64,
	bearer_token: Option<&str>,
) -> Result<Request> {
	let (token, tokenparam) = api_auth(ts);
	let url = format!("https://{}{}", domain, path);
	let request = with_browser_headers(with_api_headers(Request::get(&url)?, &token, &tokenparam));
	Ok(match bearer_token {
		Some(t) => request.header("Authorization", &format!("Bearer {t}")),
		None => request,
	})
}

fn api_post_on_domain_with_token_and_ts<T: DeserializeOwned>(
	domain: &str,
	path: &str,
	body: &[u8],
	ts: u64,
) -> Result<T> {
	let (token, tokenparam) = api_auth(ts);
	let url = format!("https://{}{}", domain, path);
	let resp = with_browser_headers(with_api_headers(Request::post(&url)?, &token, &tokenparam))
		.header("content-type", "application/x-www-form-urlencoded")
		.body(body)
		.send()?;
	parse_response(resp, ts)
}

pub fn login(username: &str, password: &str) -> Result<AuthData> {
	use aidoku::helpers::uri::encode_uri_component;
	let body = format!(
		"username={}&password={}",
		encode_uri_component(username),
		encode_uri_component(password)
	);
	let domains = fetch_domain_candidates();
	if domains.is_empty() {
		return Err(error!("域名列表为空"));
	}
	let mut last_error = None;
	for domain in domains {
		let ts = current_ts();
		match api_post_on_domain_with_token_and_ts::<AuthData>(
			&domain,
			url::login_path(),
			body.as_bytes(),
			ts,
		) {
			Ok(auth) if auth.is_valid() => return Ok(auth),
			Ok(_) => last_error = Some(error!("登录响应无效")),
			Err(err) => last_error = Some(err),
		}
	}
	Err(last_error.unwrap_or_else(|| error!("登录失败，请稍后再试")))
}
