use aidoku::{
	AidokuError, Result,
	alloc::String,
	imports::{
		defaults::{DefaultValue, defaults_get_json, defaults_set},
		net::{Request, Response},
	},
	prelude::*,
};

use crate::{models::responses::TokenResponse, settings::get_api_url};

const TOKEN_KEY: &str = "login";

const REFRESH_PATH: &str = "/api/auth/oauth/token";
const CLIENT_ID: &str = "3";
const REDIRECT_URI: &str = "ru.libapp.oauth://type/callback";
const GRANT_TYPE_REFRESH: &str = "refresh_token";

const HEADER_AUTH: &str = "Authorization";
const HEADER_CONTENT_TYPE: &str = "Content-Type";
const CONTENT_TYPE_FORM: &str = "application/x-www-form-urlencoded";
const AUTH_SCHEME: &str = "Bearer";

pub trait AuthRequest {
	fn authed(self) -> Result<Response>;
}

impl AuthRequest for Request {
	fn authed(mut self) -> Result<Response> {
		if let Ok(token) = get_token()
			&& let Some(access_token) = token.access_token
		{
			self = self.header(HEADER_AUTH, &format!("{AUTH_SCHEME} {access_token}"));
		}

		let response = self.send()?;

		// Try refresh and retry once
		if response.status_code() == 401
			&& refresh_token().is_ok()
			&& let Ok(new_token) = get_token()
			&& let Some(access_token) = new_token.access_token
		{
			return Ok(response
				.into_request()
				.header(HEADER_AUTH, &format!("{AUTH_SCHEME} {access_token}"))
				.send()?);
		}

		Ok(response)
	}
}

/// Retrieves the stored authentication token from defaults.
fn get_token() -> Result<TokenResponse> {
	defaults_get_json::<TokenResponse>(TOKEN_KEY)
		.map_err(|_| AidokuError::Message("No token".into()))
}

/// Stores the authentication token JSON string into defaults.
fn set_token(token_json: String) {
	defaults_set(TOKEN_KEY, DefaultValue::String(token_json));
}

/// Attempts to refresh the authentication token using the stored refresh token.
fn refresh_token() -> Result<()> {
	let current = get_token()?;
	let refresh_token = current
		.refresh_token
		.ok_or_else(|| AidokuError::Message("No refresh token".into()))?;

	let body = format!(
		"grant_type={GRANT_TYPE_REFRESH}&client_id={CLIENT_ID}&refresh_token={refresh_token}&redirect_uri={REDIRECT_URI}",
	);

	let response = Request::post(format!("{}{REFRESH_PATH}", get_api_url()))?
		.header(HEADER_CONTENT_TYPE, CONTENT_TYPE_FORM)
		.body(body)
		.send()?;

	if response.status_code() == 200 {
		let data = String::from_utf8(response.get_data()?).unwrap_or_default();
		set_token(data);
	}

	Ok(())
}

#[cfg(test)]
mod test;
