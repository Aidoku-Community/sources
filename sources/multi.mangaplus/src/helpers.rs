use aidoku::{
	alloc::{String, string::ToString},
	imports::std::current_date,
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

// generates a pseudo random uuid using current unix timestamp
// https://en.wikipedia.org/wiki/Linear_congruential_generator
pub fn uuid() -> String {
	let mut seed = current_date() as u64;
	let mut bytes = [0u8; 16];

	const A: u64 = 1664525;
	const C: u64 = 1013904223;
	for chunk in bytes.chunks_mut(4) {
		seed = seed.wrapping_mul(A).wrapping_add(C);
		let bytes = seed.to_le_bytes();
		for (i, b) in chunk.iter_mut().enumerate() {
			*b = bytes[i];
		}
	}

	let uuid = uuid::Builder::from_random_bytes(bytes).into_uuid();
	uuid.to_string()
}
