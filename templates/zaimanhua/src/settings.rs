use crate::models::UserInfo;
use aidoku::{
	alloc::String,
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set, defaults_set_data},
		std::current_date,
	},
	serde::{Deserialize, Serialize},
};

const USERNAME_KEY: &str = "login.username";
const PASSWORD_KEY: &str = "login.password";
const TOKEN_KEY: &str = "auth_token";
const PROFILE_KEY: &str = "profile";
const AUTO_CHECKIN_KEY: &str = "autoCheckin";

pub fn is_logged_in() -> bool {
	defaults_get::<String>(USERNAME_KEY).is_some()
}

pub fn get_credentials() -> Option<(String, String)> {
	Some((defaults_get(USERNAME_KEY)?, defaults_get(PASSWORD_KEY)?))
}

pub fn get_token() -> Option<String> {
	defaults_get(TOKEN_KEY)
}

pub fn set_token(token: &str) {
	defaults_set(TOKEN_KEY, DefaultValue::String(token.into()));
}

pub fn get_auto_checkin() -> bool {
	defaults_get(AUTO_CHECKIN_KEY).unwrap_or(false)
}

#[derive(Serialize, Deserialize)]
pub struct Profile {
	pub nickname: String,
	pub level: i64,
	pub is_sign: bool,
	day: i64,
}

pub fn get_profile() -> Option<Profile> {
	defaults_get::<Profile>(PROFILE_KEY).filter(|profile| profile.day == today())
}

pub fn set_profile(info: UserInfo) {
	let profile = Profile {
		nickname: info.nickname,
		level: info.user_level,
		is_sign: info.is_sign,
		day: today(),
	};
	defaults_set_data(PROFILE_KEY, &profile);
}

pub fn logout() {
	for key in [TOKEN_KEY, PROFILE_KEY, AUTO_CHECKIN_KEY] {
		defaults_set(key, DefaultValue::Null);
	}
}

// check-in resets at midnight beijing time
fn today() -> i64 {
	(current_date() + 8 * 3600) / 86400
}
