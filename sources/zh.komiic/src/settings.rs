use super::*;

impl KomiicSource {
	pub(super) fn auth_token() -> Option<String> {
		defaults_get(TOKEN_KEY)
	}

	fn set_just_logged_in() {
		defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Bool(true));
	}

	fn is_just_logged_in() -> bool {
		defaults_get::<bool>(JUST_LOGGED_IN_KEY).unwrap_or(false)
	}

	fn clear_just_logged_in() {
		defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
	}

	fn clear_auth() {
		defaults_set(TOKEN_KEY, DefaultValue::Null);
		defaults_set(JUST_LOGGED_IN_KEY, DefaultValue::Null);
	}

	pub(super) fn prefers_books() -> bool {
		defaults_get::<bool>(PREFER_BOOKS_KEY).unwrap_or(true)
	}
}

impl BasicLoginHandler for KomiicSource {
	fn handle_basic_login(&self, _key: String, username: String, password: String) -> Result<bool> {
		let json = Self::post_json(
			LOGIN_URL,
			json!({
				"email": username,
				"password": password
			}),
		)?;
		if let Some(token) = json.get("token").and_then(Value::as_str) {
			defaults_set(TOKEN_KEY, DefaultValue::String(String::from(token)));
			Self::set_just_logged_in();
			Ok(true)
		} else {
			Ok(false)
		}
	}
}

impl NotificationHandler for KomiicSource {
	fn handle_notification(&self, notification: String) {
		if notification.as_str() == "login" {
			if Self::is_just_logged_in() {
				Self::clear_just_logged_in();
			} else {
				Self::clear_auth();
			}
		}
	}
}
