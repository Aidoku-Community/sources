use crate::{
	ACCOUNT_API_URL,
	models::{ApiResponse, LoginData, TaskListData, UserInfo, UserInfoData},
	settings,
};
use aidoku::{
	Result,
	alloc::{String, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::net::{Request, RequestError, Response},
	prelude::*,
	serde::de::{DeserializeOwned, IgnoredAny},
};

pub mod urls {
	use crate::{Kind, PAGE_SIZE, V4_API_URL};
	use aidoku::{
		alloc::{String, format},
		helpers::uri::encode_uri_component,
	};
	use core::fmt::Display;

	pub fn search(kind: Kind, keyword: &str, page: i32) -> String {
		// search drops punctuation from the keyword, which glues words like TYPE-MOON together
		let keyword = encode_uri_component(keyword.replace(|c: char| !c.is_alphanumeric(), " "));
		let source = match kind {
			Kind::Comic => "",
			Kind::Novel => "&source=1",
		};
		format!("{V4_API_URL}/search/index?keyword={keyword}{source}&page={page}&size={PAGE_SIZE}")
	}

	pub fn search_id(kind: Kind, id: impl Display) -> String {
		let path = kind.path();
		format!("{V4_API_URL}/search/unified?keyword={id}&type={path}")
	}

	pub fn list(kind: Kind, query: &str, page: i32) -> String {
		let path = kind.path();
		format!("{V4_API_URL}/{path}/filter/list?{query}&page={page}&size={PAGE_SIZE}")
	}

	pub fn detail(kind: Kind, id: impl Display) -> String {
		let path = kind.path();
		format!("{V4_API_URL}/{path}/detail/{id}")
	}

	pub fn subscriptions(kind: Kind, page: i32) -> String {
		let path = kind.path();
		format!("{V4_API_URL}/{path}/sub/list?page={page}&size={PAGE_SIZE}")
	}

	pub fn news(kind: Kind) -> String {
		let category = match kind {
			Kind::Comic => 2,
			Kind::Novel => 3,
		};
		format!("{V4_API_URL}/news/list/{category}/3/1")
	}
}

pub fn request(url: &str, token: Option<&str>) -> Result<Request> {
	// without a Platform header, some comics answer 该漫画无法观看
	let request = Request::get(url)?.header("Platform", "h5");
	Ok(match token {
		Some(token) => request.header("Authorization", &format!("Bearer {token}")),
		None => request,
	})
}

pub fn fetch<T: DeserializeOwned + Default>(url: &str, token: Option<&str>) -> Result<T> {
	request(url, token)?
		.json_owned::<ApiResponse<T>>()?
		.into_result()
}

pub fn parse<T: DeserializeOwned + Default>(
	response: Option<core::result::Result<Response, RequestError>>,
) -> Result<T> {
	response
		.ok_or_else(|| error!("请求失败"))??
		.get_json_owned::<ApiResponse<T>>()?
		.into_result()
}

// account endpoints and subscriptions answer errno 99 for an expired token
pub fn send_authed<T: DeserializeOwned>(url: &str, token: &str) -> Result<ApiResponse<T>> {
	let response: ApiResponse<T> = request(url, Some(token))?.json_owned()?;
	if response.errno == 99
		&& let Some((username, password)) = settings::get_credentials()
		&& let Ok(Some(token)) = login(&username, &password)
	{
		settings::set_token(&token);
		return request(url, Some(&token))?.json_owned();
	}
	Ok(response)
}

pub fn login(username: &str, password: &str) -> Result<Option<String>> {
	let mut body = QueryParameters::new();
	body.push("username", Some(username));
	body.push("passwd", Some(&format!("{:x}", md5::compute(password))));
	let data: LoginData = Request::post(format!("{ACCOUNT_API_URL}/login/passwd"))?
		.header("Content-Type", "application/x-www-form-urlencoded")
		.body(body.to_string())
		.json_owned::<ApiResponse<LoginData>>()?
		.into_result()?;
	Ok(data.user.and_then(|user| user.token))
}

pub fn user_info() -> Option<UserInfo> {
	let token = settings::get_token()?;
	send_authed::<UserInfoData>(&format!("{ACCOUNT_API_URL}/userInfo/get"), &token)
		.and_then(ApiResponse::into_result)
		.ok()?
		.user_info
}

pub fn sync_account() {
	let auto_checkin = settings::get_auto_checkin();
	if settings::get_profile().is_some_and(|profile| profile.is_sign || !auto_checkin) {
		return;
	}
	let Some(mut info) = user_info() else {
		return;
	};
	if auto_checkin && !info.is_sign {
		let Some(token) = settings::get_token() else {
			return;
		};
		if sign_in(&token).is_ok() {
			claim_rewards(&token);
			info.is_sign = true;
		}
	}
	settings::set_profile(info);
}

fn sign_in(token: &str) -> Result<()> {
	Request::post(format!("{ACCOUNT_API_URL}/task/sign_in"))?
		.header("Authorization", &format!("Bearer {token}"))
		.json_owned::<ApiResponse<IgnoredAny>>()?
		.into_result()
		.map(|_| ())
}

fn claim_rewards(token: &str) {
	let Ok(Some(task)) =
		send_authed::<TaskListData>(&format!("{ACCOUNT_API_URL}/task/list"), token)
			.and_then(ApiResponse::into_result)
			.map(|data| data.task)
	else {
		return;
	};
	for item in task
		.day_task
		.iter()
		.chain(&task.new_user_task)
		.chain(task.sum_sign_task.iter().flat_map(|sum| &sum.list))
		.filter(|item| item.status == 2)
	{
		let url = format!("{ACCOUNT_API_URL}/task/get_reward?task_id={}", item.id);
		_ = send_authed::<IgnoredAny>(&url, token);
	}
}
