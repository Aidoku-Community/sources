use aes::{
	Aes256,
	cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray},
};
use aidoku::{
	Result,
	alloc::{String, Vec, format},
	bail, error,
	helpers::uri::encode_uri_component,
	imports::{
		net::{Request, RequestError, Response},
		std::current_date,
	},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use core::cell::Cell;
use serde::de::DeserializeOwned;

use crate::models::{ApiOuter, AuthData, BlockState, DomainRefreshResp, SearchResp, SettingData};
use crate::settings;

const DOMAIN_REFRESH_URLS: [&str; 3] = [
	"https://rup4a04-c02.tos-cn-hongkong.bytepluses.com/newsvr-2025.txt",
	"https://rup4a04-c01.tos-ap-southeast-1.bytepluses.com/newsvr-2025.txt",
	"https://rup4a04-c03.tos-cn-beijing.bytepluses.com.cn/newsvr-2025.txt",
];
const DOMAIN_REFRESH_SECRET: &str = "diosfjckwpqpdfjkvnqQjsik";

const JM_VERSION: &str = "2.1.11";
const JM_SECRET: &str = "185Hcomic3PAPP7R";

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
	failed: Cell<bool>,
}

impl ApiContext {
	pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
		let ts = current_ts();
		decode(self.receive(self.request(path, ts)?.send())?, ts)
	}

	pub fn get_pair<A: DeserializeOwned, B: DeserializeOwned>(
		&self,
		first: &str,
		second: &str,
	) -> Result<(A, B)> {
		let ts = current_ts();
		let requests = [self.request(first, ts)?, self.request(second, ts)?];
		let mut responses = Request::send_all(requests).into_iter();
		let first = self.receive(responses.next().ok_or_else(|| error!("缺少回應"))?)?;
		let second = self.receive(responses.next().ok_or_else(|| error!("缺少回應"))?)?;
		Ok((decode(first, ts)?, decode(second, ts)?))
	}

	// list items have no tags, so blocked albums are also looked up by search
	pub fn get_list<T: DeserializeOwned>(
		&self,
		path: &str,
		order: &str,
		category: &str,
		block: &mut BlockState,
	) -> Result<T> {
		let Some(keywords) = block.lookup() else {
			return self.get(path);
		};
		let lookup = url::search(&keywords, "site", order, category, 1);
		let (list, matches): (T, SearchResp) = self.get_pair(path, &lookup)?;
		block.hide(matches.content.into_iter().map(|item| item.id));
		Ok(list)
	}

	pub fn login(&self, username: &str, password: &str) -> Result<AuthData> {
		let ts = current_ts();
		let url = format!("https://{}{}", self.domain, url::login_path());
		let body = format!(
			"username={}&password={}",
			encode_uri_component(username),
			encode_uri_component(password)
		);
		let request = with_auth(Request::post(&url)?, ts)
			.header("content-type", "application/x-www-form-urlencoded")
			.body(body.as_bytes());
		let auth: AuthData = decode(self.receive(request.send())?, ts)?;
		if !auth.is_valid() {
			bail!("登入回應無效");
		}
		Ok(auth)
	}

	pub fn is_usable(&self) -> bool {
		!self.failed.get()
	}

	fn request(&self, path: &str, ts: u64) -> Result<Request> {
		let auth = settings::get_auth();
		get_request_on_domain(
			&self.domain,
			path,
			ts,
			auth.as_ref().map(|auth| auth.jwt_token.as_str()),
		)
	}

	fn receive(&self, resp: core::result::Result<Response, RequestError>) -> Result<ApiOuter> {
		envelope(resp).inspect_err(|_| self.failed.set(true))
	}
}

pub fn context(failed: Option<&ApiContext>) -> Result<ApiContext> {
	let ts = current_ts();
	let mut domains = fetch_domain_candidates();
	if let Some(i) = failed.and_then(|api| domains.iter().position(|d| *d == api.domain)) {
		domains.rotate_left(i + 1);
	}
	for domain in domains {
		for shunt in PREFERRED_IMAGE_SHUNTS {
			let setting = get_request_on_domain(&domain, &url::setting_path(shunt), ts, None)
				.and_then(|request| envelope(request.send()))
				.and_then(|outer| decode::<SettingData>(outer, ts));
			if let Ok(setting) = setting
				&& let Some(cdn_base) = setting.img_host.as_deref().and_then(normalize_cdn_base)
			{
				return Ok(ApiContext {
					domain,
					cdn_base,
					failed: Cell::default(),
				});
			}
		}
	}
	bail!("目前所有網域都暫時無法使用")
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

	pub fn search(
		query: &str,
		search_type: &str,
		order: &str,
		category: &str,
		page: i32,
	) -> String {
		let q = encode_uri_component(query);
		if category.is_empty() {
			format!("/search?search_query={q}&search_type={search_type}&o={order}&page={page}")
		} else {
			let c = encode_uri_component(category);
			format!(
				"/search?search_query={q}&search_type={search_type}&o={order}&c={c}&page={page}"
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

	pub fn latest(page: i32) -> String {
		format!("/latest?page={page}")
	}

	pub fn promote_list(id: &str, page: i32) -> String {
		format!("/promote_list?id={id}&page={page}")
	}

	pub fn novels() -> &'static str {
		"/novels?o=mr&t=a"
	}

	pub fn search_novels(query: &str, order: &str) -> String {
		let q = encode_uri_component(query);
		format!("/search_novels?search_query={q}&o={order}")
	}

	pub fn novel(id: &str) -> String {
		format!("/novel?nid={id}")
	}

	pub fn novel_chapter(id: &str) -> String {
		format!("/novelchapters?ncid={id}")
	}

	pub fn setting_path(shunt: u8) -> String {
		format!("/setting?app_img_shunt={shunt}")
	}

	pub fn login_path() -> &'static str {
		"/login"
	}
}

fn md5_hex(input: &[u8]) -> String {
	format!("{:x}", md5::compute(input))
}

fn current_ts() -> u64 {
	current_date() as u64
}

fn with_auth(request: Request, ts: u64) -> Request {
	request
		.header("token", &md5_hex(format!("{ts}{JM_SECRET}").as_bytes()))
		.header("tokenparam", &format!("{ts},{JM_VERSION}"))
}

fn derive_aes_key(secret: &str) -> [u8; 32] {
	let hex = md5_hex(secret.as_bytes());
	let mut key = [0u8; 32];
	key.copy_from_slice(hex.as_bytes());
	key
}

fn aes256_ecb_decrypt(mut data: Vec<u8>, key: &[u8; 32]) -> Result<Vec<u8>> {
	if data.is_empty() || !data.len().is_multiple_of(16) {
		bail!("回應資料長度異常：{}", data.len());
	}
	let cipher = Aes256::new_from_slice(key).map_err(|_| error!("解密器初始化失敗"))?;
	for chunk in data.as_chunks_mut::<16>().0 {
		cipher.decrypt_block(GenericArray::from_mut_slice(chunk));
	}
	let pad = *data.last().ok_or_else(|| error!("解密結果為空"))? as usize;
	if pad == 0 || pad > 16 || data.len() < pad {
		bail!("回應填充資料異常：{}", pad);
	}
	data.truncate(data.len() - pad);
	Ok(data)
}

fn decrypt_data(data_b64: &str, ts: u64) -> Result<Vec<u8>> {
	let key = derive_aes_key(&format!("{ts}{JM_SECRET}"));
	let ct = STANDARD
		.decode(data_b64.as_bytes())
		.map_err(|_| error!("回應解碼失敗"))?;
	let mut json = aes256_ecb_decrypt(ct, &key)?;
	let start = json
		.iter()
		.position(|b| matches!(b, b'{' | b'['))
		.unwrap_or(0);
	let end = json
		.iter()
		.rposition(|b| matches!(b, b'}' | b']'))
		.map_or(json.len(), |i| i + 1);
	if start >= end {
		bail!("回應邊界異常");
	}
	json.truncate(end);
	json.drain(..start);
	Ok(json)
}

fn envelope(resp: core::result::Result<Response, RequestError>) -> Result<ApiOuter> {
	let body = resp?.get_string()?;
	ApiOuter::parse(&body).map_err(|_| error!("外層回應解析失敗"))
}

fn decode<T: DeserializeOwned>(outer: ApiOuter, ts: u64) -> Result<T> {
	let Some(data) = outer.data else {
		match outer.error_msg.filter(|msg| !msg.is_empty()) {
			Some(msg) => bail!("{msg}"),
			None => bail!("外層回應解析失敗"),
		}
	};
	serde_json::from_slice(&decrypt_data(&data, ts)?).map_err(|_| error!("內容資料解析失敗"))
}

fn fetch_domain_candidates() -> Vec<String> {
	let key = derive_aes_key(DOMAIN_REFRESH_SECRET);
	for url in DOMAIN_REFRESH_URLS {
		let Some(ct_b64) = Request::get(url).ok().and_then(|r| r.string().ok()) else {
			continue;
		};
		let ct_b64 = ct_b64.trim().trim_start_matches('\u{feff}');
		let Ok(ct) = STANDARD.decode(ct_b64.as_bytes()) else {
			continue;
		};
		let Ok(raw) = aes256_ecb_decrypt(ct, &key) else {
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

fn get_request_on_domain(
	domain: &str,
	path: &str,
	ts: u64,
	bearer_token: Option<&str>,
) -> Result<Request> {
	let url = format!("https://{}{}", domain, path);
	let request = with_auth(Request::get(&url)?, ts);
	Ok(match bearer_token {
		Some(t) => request.header("Authorization", &format!("Bearer {t}")),
		None => request,
	})
}
