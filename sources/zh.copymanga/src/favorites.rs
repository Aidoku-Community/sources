use crate::{
	auth::{AuthedRequest as _, try_relogin},
	net::{Url, base_url, manga_url},
};

const BUTTON_HOST: &str = "https://www.copy5000.com";

#[cfg(test)]
pub(crate) const fn button_host() -> &'static str {
	BUTTON_HOST
}
use aidoku::{
	DeepLinkResult, Manga, MangaPageResult, MangaStatus, Result,
	alloc::{String, Vec, format, vec},
	bail, error,
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		error::AidokuError,
		net::{Request, Response},
	},
	serde::Deserialize,
};

const PAGE_LIMIT: i32 = 50;

/// free_type: 网站书架的「免費」(1) 与「付費」(2) 两个标签页。
#[derive(Clone, Copy)]
pub enum CollectType {
	Free,
	Charged,
}

impl CollectType {
	pub fn param(self) -> &'static str {
		match self {
			Self::Free => "1",
			Self::Charged => "2",
		}
	}
}

#[derive(Deserialize)]
struct CollectResponse {
	#[serde(default)]
	results: Option<CollectResults>,
}

#[derive(Deserialize)]
struct CollectResults {
	#[serde(default)]
	list: Vec<CollectItem>,
	#[serde(default)]
	total: i32,
	#[serde(default)]
	offset: i32,
	#[serde(default)]
	limit: i32,
}

#[derive(Deserialize)]
struct CollectItem {
	#[serde(default)]
	comic: Option<CollectComic>,
}

#[derive(Deserialize)]
struct CollectComic {
	#[serde(default)]
	path_word: String,
	#[serde(default)]
	name: String,
	#[serde(default)]
	cover: Option<String>,
	#[serde(default)]
	status: Option<u8>,
	#[serde(default)]
	author: Option<Vec<Author>>,
}

#[derive(Deserialize)]
struct Author {
	#[serde(default)]
	name: String,
}

fn to_manga(comic: CollectComic) -> Manga {
	let url = manga_url(&comic.path_word).ok();
	let status = match comic.status {
		Some(0) => MangaStatus::Ongoing,
		Some(1 | 2) => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	let cover = comic.cover.map(|cover| cover.replace(".328x422.jpg", ""));
	let authors = comic
		.author
		.map(|authors| authors.into_iter().map(|a| a.name).collect());
	Manga {
		key: comic.path_word,
		title: comic.name,
		cover,
		authors,
		url,
		status,
		..Default::default()
	}
}

fn fetch_collect_page(page: i32, collect_type: CollectType) -> Result<MangaPageResult> {
	let offset = (page - 1).max(0) * PAGE_LIMIT;
	let url = format!(
		"{}/api/v3/member/collect/comics?limit={PAGE_LIMIT}&offset={offset}&free_type={}&ordering=-datetime_created",
		base_url()?,
		collect_type.param(),
	);

	let mut response = Request::get(&url)?.authed()?.send()?;
	if response.status_code() == 401 && try_relogin() {
		response = Request::get(&url)?.authed()?.send()?;
	}
	if response.status_code() == 401 {
		bail!("登录已失效，请重新在设置中登录");
	}

	let body = response.get_string()?;
	let parsed: CollectResponse =
		serde_json::from_str(&body).map_err(|_| error!("收藏响应解析失败"))?;
	let results = parsed
		.results
		.ok_or_else(|| error!("收藏响应缺少 results"))?;

	let entries = results
		.list
		.into_iter()
		.filter_map(|item| item.comic)
		.filter(|comic| !comic.path_word.is_empty())
		.map(to_manga)
		.collect();

	let has_next_page = results.offset + results.limit < results.total;

	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

/// 拉取一页账号收藏（免費+付費合并，各取同页码后拼接）。
/// 每次调用都实时请求网站，收藏变化无需缓存失效逻辑。
pub fn collect_page(page: i32) -> Result<MangaPageResult> {
	let free = fetch_collect_page(page, CollectType::Free)?;
	merge_collect_pages(free, fetch_collect_page(page, CollectType::Charged))
}

/// 合并免费、付费书架。免费请求已成功时，付费请求失败不能将其替换为错误。
pub(crate) fn merge_collect_pages(
	mut free: MangaPageResult,
	charged: Result<MangaPageResult>,
) -> Result<MangaPageResult> {
	if let Ok(charged) = charged {
		free.has_next_page = free.has_next_page || charged.has_next_page;
		free.entries.extend(charged.entries);
	}
	Ok(free)
}

fn error_text(err: &AidokuError) -> String {
	match err {
		AidokuError::Message(message) => message.clone(),
		other => format!("{other:?}"),
	}
}

/// 从详情页 HTML 提取收藏按钮上的漫画 UUID（onclick="collect('...')"）。
pub fn resolve_comic_uuid(path_word: &str) -> Result<String> {
	let html = Url::manga(path_word).request()?.string()?;
	let marker = "collect('";
	let Some(start) = html.find(marker) else {
		bail!("详情页中未找到收藏标识，漫画可能不存在");
	};
	let rest = &html[start + marker.len()..];
	let Some(end) = rest.find('\'') else {
		bail!("详情页收藏标识解析失败");
	};
	let uuid = &rest[..end];
	if uuid.is_empty() {
		bail!("详情页收藏标识为空");
	}
	Ok(uuid.into())
}

// 收藏状态统一存单键 JSON map（登出时整键清除，避免换号串状态）：
// favStateMap = { "{path_word}": { "collected": bool, "msg": "一次性反馈(读后即删)" } }
const FAV_STATE_MAP_KEY: &str = "favStateMap";

fn load_state_map() -> serde_json::Map<String, serde_json::Value> {
	defaults_get::<String>(FAV_STATE_MAP_KEY)
		.and_then(|s| serde_json::from_str(&s).ok())
		.unwrap_or_default()
}

fn save_state_map(map: &serde_json::Map<String, serde_json::Value>) {
	if let Ok(s) = serde_json::to_string(map) {
		defaults_set(FAV_STATE_MAP_KEY, DefaultValue::String(s));
	}
}

/// 登出/换号时整体清除收藏状态缓存。
pub(crate) fn clear_all_state() {
	// 空对象同样表示无缓存，且避免依赖不同 Aidoku defaults 实现对 Null 删除语义的一致性。
	defaults_set(FAV_STATE_MAP_KEY, DefaultValue::String(String::from("{}")));
}

/// 按钮状态缓存：None=从未判断过（打开时应实时扫描书架）；
/// Some(true/false)=已有判断结果。只反映本 App 内的操作，
/// 网站上做的改动会在下次点击时自动纠正。
pub(crate) fn get_collected_state(path_word: &str) -> Option<bool> {
	load_state_map()
		.get(path_word)
		.and_then(|entry| entry.get("collected"))
		.and_then(serde_json::Value::as_bool)
}

pub(crate) fn set_collected_state(path_word: &str, collected: bool) {
	let mut map = load_state_map();
	let entry = map
		.entry(String::from(path_word))
		.or_insert(serde_json::Value::Null);
	if !entry.is_object() {
		*entry = serde_json::Value::Object(serde_json::Map::new());
	}
	if let Some(obj) = entry.as_object_mut() {
		obj.insert(
			String::from("collected"),
			serde_json::Value::Bool(collected),
		);
	}
	save_state_map(&map);
}

/// 一次性反馈消息：读取即清除，只在动作后的刷新页显示一次，之后不再出现。
pub(crate) fn set_fav_msg(path_word: &str, text: &str) {
	let mut map = load_state_map();
	let entry = map
		.entry(String::from(path_word))
		.or_insert(serde_json::Value::Null);
	if !entry.is_object() {
		*entry = serde_json::Value::Object(serde_json::Map::new());
	}
	if let Some(obj) = entry.as_object_mut() {
		obj.insert(
			String::from("msg"),
			serde_json::Value::String(String::from(text)),
		);
	}
	save_state_map(&map);
}

pub(crate) fn take_fav_msg(path_word: &str) -> Option<String> {
	let mut map = load_state_map();
	let message = map
		.get_mut(path_word)
		.and_then(|entry| entry.as_object_mut())
		.and_then(|obj| obj.remove("msg"))
		.and_then(|value| value.as_str().map(String::from));
	if message.is_some() {
		save_state_map(&map);
	}
	message
}

/// 检查漫画是否已在網站書架（按用户设定：只扫免費书架，最多 10 页 / 500 条；
/// 付费书架不扫——本源收藏的漫画均在免费列表）。
/// 返回：Some(true/false)=扫描结论；None=扫描失败（调用方不应缓存该结果）。
/// 新添加的漫画按收藏时间倒序排在最前，通常第 1 页即可命中。
fn is_collected(path_word: &str) -> Option<bool> {
	const MAX_PAGES: i32 = 10;
	for page in 1..=MAX_PAGES {
		match fetch_collect_page(page, CollectType::Free) {
			Ok(result) => {
				if result.entries.iter().any(|m| m.key == path_word) {
					return Some(true);
				}
				if !result.has_next_page {
					return Some(false);
				}
			}
			Err(_) => return None,
		}
	}
	Some(false)
}

/// 簡介收藏按鈕入口（handle_deep_link 路由 /__fav/{add|remove}/{path_word}）。
/// 执行写操作并记录状态，返回当前漫画让 App 刷新详情页展示结果。
pub fn deep_link_favorite(path_word: &str, add: bool) -> Result<Option<DeepLinkResult>> {
	let action = if add { "收藏漫画" } else { "取消收藏" };

	let result = favorite_with_state(path_word, add);

	match &result {
		Ok(message) => {
			let mark = if message.starts_with("已在") || message.starts_with("尚未") {
				"ℹ️"
			} else {
				"✅"
			};
			set_fav_msg(path_word, &format!("{mark} {message}"));
		}
		Err(err) => {
			set_fav_msg(path_word, &format!("❌ {action}失败：{}", error_text(err)));
		}
	}
	// 无论如何返回当前漫画：App 会重新拉取详情并推入刷新页，用户即可看到状态
	Ok(Some(DeepLinkResult::Manga {
		key: String::from(path_word),
	}))
}

pub(crate) const fn favorite_operation_needed(add: bool, collected: bool) -> bool {
	add != collected
}

/// 两个收藏入口共用的状态感知路径。
/// 扫描失败时不写入本地状态，仍执行用户明确请求的写操作并如实返回其结果。
fn favorite_with_state(path_word: &str, add: bool) -> Result<String> {
	let Some(collected) = is_collected(path_word) else {
		return favorite_core(path_word, add);
	};
	if favorite_operation_needed(add, collected) {
		return favorite_core(path_word, add);
	}
	set_collected_state(path_word, collected);
	Ok(if collected {
		String::from("已收藏（无需重复操作）")
	} else {
		String::from("尚未收藏（无需取消）")
	})
}

/// 将详情页的收藏与评论入口合成一行，避免它们在简介顶部显得零散。
pub(crate) fn detail_action_line(favorite: Option<&str>, uuid: Option<&str>) -> Option<String> {
	let mut actions = Vec::new();
	if let Some(uuid) = uuid.filter(|uuid| !uuid.is_empty()) {
		actions.push(format!(
			"[💬 评论区](https://www.copy5000.com/h5/commentList?comicId={uuid})"
		));
	}
	if let Some(favorite) = favorite {
		actions.push(String::from(favorite));
	}
	(!actions.is_empty()).then(|| actions.join(" · "))
}

/// 簡介頂部注入收藏按鈕與評論區鏈接（Markdown）。
/// 收藏按鈕受登入和設定控制；評論區只要詳情頁有 UUID 且設定開啟就顯示。
pub fn decorate_description(
	path_word: &str,
	uuid: Option<&str>,
	description: &str,
) -> Option<String> {
	let favorite_enabled = defaults_get::<bool>("favButtons.inDetail").unwrap_or(true);
	let comment_enabled = defaults_get::<bool>("commentButtons.inDetail").unwrap_or(true);
	let mut lines: Vec<String> = Vec::new();

	let favorite = if favorite_enabled && crate::auth::is_logged_in() {
		// 一次性结果横幅（只在动作后的刷新页出现一次，之后不再出现）
		if let Some(message) = take_fav_msg(path_word) {
			lines.push(format!("## {message}"));
		}
		// 按当前收藏状态只显示一个按钮。
		// 状态未知（首次打开该漫画）时实时扫描書架判断（逻辑图第一步「先判断该漫画有没有被收藏」），
		// 结果写入缓存后不再重复扫描。
		let collected = match get_collected_state(path_word) {
			Some(state) => state,
			None => match is_collected(path_word) {
				Some(state) => {
					set_collected_state(path_word, state);
					state
				}
				// 扫描失败：不缓存，按未收藏展示，下次打开重试
				None => false,
			},
		};
		if collected {
			Some(format!(
				"[✖ 取消收藏]({BUTTON_HOST}/__fav/remove/{path_word})"
			))
		} else {
			Some(format!(
				"[➕ 收藏漫画]({BUTTON_HOST}/__fav/add/{path_word})"
			))
		}
	} else {
		None
	};

	let comment_uuid = comment_enabled.then_some(uuid).flatten();
	if let Some(actions) = detail_action_line(favorite.as_deref(), comment_uuid) {
		lines.push(actions);
	}

	if lines.is_empty() {
		return None;
	}
	let mut out = lines.join("\n\n");
	if !description.is_empty() {
		out.push_str("\n\n———\n\n");
		out.push_str(description);
	}
	Some(out)
}

/// 收藏动作的详情页核心路径。
fn favorite_core(path_word: &str, add: bool) -> Result<String> {
	let uuid = resolve_comic_uuid(path_word)?;
	set_collect(&uuid, add)?;
	let status = format!(
		"{}{path_word}",
		if add {
			"已收藏漫画: "
		} else {
			"已取消收藏: "
		}
	);
	set_collected_state(path_word, add);
	Ok(status)
}

/// 写操作端点（网站收藏/取消收藏）。Aidoku Source API 没有漫画页自定义操作钩子，
/// 因此由详情页 Markdown deep link 触发。
///
/// v21 失败原因：www 主域对部分 /api 路径返回 HTTP 200 的「服務器升級中」HTML 拦截页
/// （评论接口同病），旧实现只检查 401，把拦截页当成功。现在：
/// 依次尝试 H5 核心 API 域与当前所选主域；解析响应 JSON 的 code/message；
/// 非 JSON（拦截页）视为该域名失败并继续尝试下一个候选域名。
pub fn set_collect(comic_uuid: &str, collect: bool) -> Result<()> {
	const COLLECT_PATH: &str = "/api/v2/web/collect";
	let body = format!(
		"comic_id={}&is_collect={}",
		comic_uuid,
		if collect { "1" } else { "0" }
	);
	let hosts = collect_hosts()?;
	let mut last_error = None;
	for host in &hosts {
		let url = format!("{host}{COLLECT_PATH}");
		match set_collect_once(&url, &body) {
			Ok(()) => return Ok(()),
			Err(err) => {
				last_error = Some(err);
			}
		}
	}
	Err(last_error.unwrap_or_else(|| error!("收藏写入失败：所有接口均不可用")))
}

/// 收藏写接口候选域：H5 应用核心域优先，回退当前所选主域。
fn collect_hosts() -> Result<Vec<String>> {
	Ok(vec![String::from("https://api.copy4000.com"), base_url()?])
}

fn set_collect_once(url: &str, body: &str) -> Result<()> {
	// 单登录设计：使用 API 登录 token 的 Authorization 头和 Cookie 双通道；401 时用
	// App 保存的账密静默重登续期。
	let send = |url: String, token: &str| -> Result<Response> {
		Ok(Request::post(&url)?
			.header(
				"Content-Type",
				"application/x-www-form-urlencoded;charset=UTF-8",
			)
			.header("X-Requested-With", "XMLHttpRequest")
			.header("Cookie", &format!("token={token}"))
			.header("Authorization", &format!("Token {token}"))
			.body(body)
			.send()?)
	};

	let token = crate::auth::token().ok_or_else(|| error!("请先在设置中登录"))?;
	let mut response = send(url.into(), &token)?;
	if response.status_code() == 401
		&& try_relogin()
		&& let Some(fresh) = crate::auth::token()
	{
		response = send(url.into(), &fresh)?;
	}
	if response.status_code() == 401 {
		return Err(error!(
			"登录已失效，请在设置中重新登录（会自动续期，无需网页登录）"
		));
	}
	if response.status_code() != 200 {
		return Err(error!("HTTP {}", response.status_code()));
	}
	let resp_body = response.get_string()?;
	// 「服務器升級中」拦截页是 HTTP 200 + HTML → JSON 解析失败视为该域名不可用
	let value: serde_json::Value = serde_json::from_str(&resp_body)
		.map_err(|_| error!("响应不是 JSON（可能被主域拦截页接管）"))?;
	let code = value.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
	if code != 200 {
		let message = value
			.get("message")
			.and_then(|v| v.as_str())
			.unwrap_or("未知错误");
		return Err(error!("网站返回 {code}：{message}"));
	}
	Ok(())
}
