use crate::settings::{API_V1, API_V2, SITE_URL, USER_AGENT, settings_access_token};
use aidoku::{
	HashMap, Result,
	alloc::{String, Vec, format},
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		net::Request,
	},
};
use serde::Deserialize;

const TOKEN_KEY: &str = "remanga.token";
const USERNAME_KEY: &str = "remanga.username";
const AUTH_HINT_KEY: &str = "remanga.auth_hint";
const JUST_LOGGED_IN_KEY: &str = "remanga.just";
const BALANCE_KEY: &str = "remanga.balance";

/// Cookie names from Remanga `Re` enum / BFF credentials.
/// JS sets `token`; BFF may also set httpOnly `serverTokenV2`.
const TOKEN_COOKIE_NAMES: &[&str] = &[
	"token",
	"serverTokenV2",
	"serverToken",
	"server-token",
	"server_token",
	"access_token",
];

#[derive(Deserialize)]
struct UserEnvelope {
	content: Option<UserContent>,
}

#[derive(Deserialize)]
struct UserContent {
	username: Option<String>,
}

#[derive(Deserialize)]
struct BalanceResponse {
	balance: Option<FlexNumber>,
	amount: Option<FlexNumber>,
	value: Option<FlexNumber>,
	balance_free: Option<FlexNumber>,
	balance_paid: Option<FlexNumber>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FlexNumber {
	Int(i64),
	Float(f64),
	Text(String),
}

impl FlexNumber {
	fn as_display(&self) -> String {
		match self {
			Self::Int(v) => format!("{v}"),
			Self::Float(v) => format!("{v}"),
			Self::Text(v) => v.clone(),
		}
	}
}

/// True when a validated session token is stored.
pub fn is_logged_in() -> bool {
	session_token().is_some()
}

/// Short hint shown in DynamicSettings after a failed login attempt.
pub fn auth_hint() -> Option<String> {
	defaults_get::<String>(AUTH_HINT_KEY).filter(|s| !s.is_empty())
}

fn set_auth_hint(message: &str) {
	defaults_set(AUTH_HINT_KEY, DefaultValue::String(message.into()));
}

fn clear_auth_hint() {
	defaults_set(AUTH_HINT_KEY, DefaultValue::Null);
}

fn session_token() -> Option<String> {
	defaults_get::<String>(TOKEN_KEY).filter(|s| !s.trim().is_empty())
}

/// Bearer token for API calls (session first, then settings fallback).
pub fn access_token() -> Option<String> {
	session_token().or_else(|| normalize_token_value(&settings_access_token()?))
}

/// Username from the last successful `/users/current/` refresh.
pub fn stored_username() -> Option<String> {
	defaults_get::<String>(USERNAME_KEY).filter(|s| !s.is_empty())
}

/// Formatted lightning balance for the settings status footer.
pub fn stored_balance() -> Option<String> {
	defaults_get::<String>(BALANCE_KEY).filter(|s| !s.is_empty())
}

fn set_just_logged_in() {
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
}

/// Consumes the one-shot flag so the login notification does not clear a fresh session.
pub fn take_just_logged_in() -> bool {
	let flag = defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false);
	if flag {
		defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
	}
	flag
}

/// Clears token, username, balance, and auth hints.
pub fn logout() {
	defaults_set(TOKEN_KEY, DefaultValue::Null);
	defaults_set(USERNAME_KEY, DefaultValue::Null);
	defaults_set(BALANCE_KEY, DefaultValue::Null);
	defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
	clear_auth_hint();
}

fn store_session(token: &str, username: Option<&str>) {
	defaults_set(TOKEN_KEY, DefaultValue::String(token.into()));
	if let Some(name) = username.filter(|s| !s.is_empty()) {
		defaults_set(USERNAME_KEY, DefaultValue::String(name.into()));
	}
	clear_auth_hint();
	set_just_logged_in();
}

fn normalize_token_value(raw: &str) -> Option<String> {
	let trimmed = raw.trim();
	if trimmed.is_empty() {
		return None;
	}
	let token = trimmed
		.strip_prefix("Bearer ")
		.or_else(|| trimmed.strip_prefix("bearer "))
		.unwrap_or(trimmed)
		.trim();
	if token.is_empty() || token.starts_with('{') {
		None
	} else {
		Some(token.into())
	}
}

fn looks_like_token(value: &str) -> bool {
	let v = value.trim();
	if v.len() < 20 || v.starts_with('{') || v.contains(' ') {
		return false;
	}
	v.chars()
		.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '='))
}

/// Adds Remanga JSON headers and optional `Authorization: Bearer`.
pub trait AuthedRequest {
	fn remanga(self) -> Self;
}

impl AuthedRequest for Request {
	fn remanga(self) -> Self {
		let req = self
			.header("User-Agent", USER_AGENT)
			.header("Referer", &format!("{SITE_URL}/"))
			.header("Origin", SITE_URL)
			.header("Accept", "application/json");
		if let Some(token) = access_token() {
			req.header("Authorization", &format!("Bearer {token}"))
		} else {
			req
		}
	}
}

fn validate_token(token: &str) -> Result<Option<Option<String>>> {
	let response = Request::get(format!("{API_V1}/users/current/"))?
		.header("User-Agent", USER_AGENT)
		.header("Referer", &format!("{SITE_URL}/"))
		.header("Origin", SITE_URL)
		.header("Accept", "application/json")
		.header("Authorization", &format!("Bearer {token}"))
		.send()?;
	if response.status_code() != 200 {
		return Ok(None);
	}
	let Ok(user) = response.get_json_owned::<UserEnvelope>() else {
		return Ok(None);
	};
	let Some(content) = user.content else {
		return Ok(None);
	};
	Ok(Some(content.username))
}

fn try_adopt_token(token: &str) -> Result<bool> {
	let Some(token) = normalize_token_value(token) else {
		return Ok(false);
	};
	let Some(username) = validate_token(&token)? else {
		return Ok(false);
	};
	store_session(&token, username.as_deref());
	let _ = refresh_account_info();
	Ok(true)
}

/// Adopts a Bearer token from WebView cookies (`token` / `serverTokenV2`).
pub fn handle_web_login(cookies: HashMap<String, String>) -> Result<bool> {
	for name in TOKEN_COOKIE_NAMES {
		if let Some(value) = cookies.get(*name)
			&& try_adopt_token(value)?
		{
			return Ok(true);
		}
	}

	for (name, value) in cookies.iter() {
		let lower = name.to_ascii_lowercase();
		if TOKEN_COOKIE_NAMES
			.iter()
			.any(|n| n.eq_ignore_ascii_case(&lower))
			&& try_adopt_token(value)?
		{
			return Ok(true);
		}
	}

	let mut candidates: Vec<&String> = cookies
		.iter()
		.filter(|(name, value)| {
			let lower = name.to_ascii_lowercase();
			!lower.contains("user") && looks_like_token(value)
		})
		.map(|(_, value)| value)
		.collect();
	candidates.sort_by_key(|v| core::cmp::Reverse(v.len()));
	for value in candidates {
		if try_adopt_token(value)? {
			return Ok(true);
		}
	}

	set_auth_hint("Токен не найден. Завершите вход на сайте и закройте окно.");
	Ok(false)
}

/// Validates and stores the manual token from settings.
pub fn apply_token_from_settings() -> Result<bool> {
	let Some(raw) = settings_access_token() else {
		return Ok(false);
	};
	if try_adopt_token(&raw)? {
		Ok(true)
	} else {
		set_auth_hint("Неверный или просроченный токен");
		Ok(false)
	}
}

/// Refreshes username and lightning balance for the settings status footer.
pub fn refresh_account_info() -> Result<()> {
	let Some(_) = access_token() else {
		return Ok(());
	};

	if let Ok(user) = Request::get(format!("{API_V1}/users/current/"))?
		.remanga()
		.json_owned::<UserEnvelope>()
		&& let Some(content) = user.content
		&& let Some(name) = content.username.filter(|s| !s.is_empty())
	{
		defaults_set(USERNAME_KEY, DefaultValue::String(name));
	}

	if let Ok(bal) = Request::get(format!("{API_V2}/billing/lightning-balance/"))?
		.remanga()
		.json_owned::<BalanceResponse>()
	{
		let text = match (bal.balance_free, bal.balance_paid) {
			(Some(free), Some(paid)) => Some(format!(
				"бесплатно {} · куплено {}",
				free.as_display(),
				paid.as_display()
			)),
			(Some(free), None) => Some(format!("бесплатно {}", free.as_display())),
			(None, Some(paid)) => Some(format!("куплено {}", paid.as_display())),
			(None, None) => bal
				.balance
				.or(bal.amount)
				.or(bal.value)
				.map(|v| v.as_display()),
		};
		if let Some(text) = text.filter(|s| !s.is_empty()) {
			defaults_set(BALANCE_KEY, DefaultValue::String(text));
		}
	}

	Ok(())
}
