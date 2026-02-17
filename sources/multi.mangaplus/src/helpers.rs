use aidoku::{
	alloc::{String, string::ToString},
	prelude::format,
};

use crate::settings;
use crate::{MOBILE_API_URL, WEB_API_URL};

pub fn get_api_url() -> String {
	if settings::get_mobile() {
		MOBILE_API_URL.to_string()
	} else {
		WEB_API_URL.to_string()
	}
}

pub fn build_auth_params() -> String {
	if settings::get_mobile() {
		format!(
			"&os={}&os_ver={}&app_ver={}&secret={}",
			settings::get_os(),
			settings::get_os_ver(),
			settings::get_app_ver(),
			settings::get_secret()
		)
	} else {
		String::new()
	}
}
