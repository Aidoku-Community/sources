use crate::models::Language;
use aidoku::{
	Result,
	alloc::{string::String, vec::Vec},
	imports::defaults::defaults_get,
	prelude::*,
};

// settings keys
const LANGUAGES_KEY: &str = "languages";
const IMAGE_QUALITY_KEY: &str = "imgQuality";
const SPLIT_KEY: &str = "split";
const MOBILE_KEY: &str = "mobile";
const OS_KEY: &str = "os";
const OS_VER_KEY: &str = "osVer";
const APP_VER_KEY: &str = "appVer";
const SECRET_KEY: &str = "secret";

pub fn get_languages() -> Result<Vec<Language>> {
	defaults_get::<Vec<String>>(LANGUAGES_KEY)
		.map(|langs| {
			langs
				.into_iter()
				.map(|lang| match lang.as_str() {
					"en" => Language::English,
					"es" => Language::Spanish,
					"fr" => Language::French,
					"id" => Language::Indonesian,
					"pt-BR" => Language::BrazilianPortuguese,
					"ru" => Language::Russian,
					"th" => Language::Thai,
					"vi" => Language::Vietnamese,
					"de" => Language::German,
					_ => Language::English,
				})
				.collect()
		})
		.ok_or(error!("Unable to fetch languages"))
}

pub fn get_image_quality() -> String {
	defaults_get::<String>(IMAGE_QUALITY_KEY).unwrap_or_default()
}

pub fn get_split() -> bool {
	defaults_get::<bool>(SPLIT_KEY).unwrap_or(false)
}

pub fn get_mobile() -> bool {
	defaults_get::<bool>(MOBILE_KEY).unwrap_or(false)
}

pub fn get_os() -> String {
	defaults_get::<String>(OS_KEY).unwrap_or_default()
}

pub fn get_os_ver() -> String {
	defaults_get::<String>(OS_VER_KEY).unwrap_or_default()
}

pub fn get_app_ver() -> String {
	defaults_get::<String>(APP_VER_KEY).unwrap_or_default()
}

pub fn get_secret() -> String {
	defaults_get::<String>(SECRET_KEY).unwrap_or_default()
}
