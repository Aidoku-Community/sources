use crate::TokenResponse;
use aidoku::{
	Result,
	alloc::{string::String, vec::Vec},
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_get_json, defaults_set},
		error::AidokuError,
	},
};
use core::fmt::Write;

// settings keys
const LANGUAGES_KEY: &str = "languages";
const TITLE_PREFERENCE_KEY: &str = "titlePreference";
const COVER_QUALITY_KEY: &str = "coverQuality";
const CONTENT_RATING_KEY: &str = "contentRating";
const BLOCKED_UUIDS_KEY: &str = "blockedUUIDs";
const FORCE_PORT_KEY: &str = "standardHttpsPort";
const DATA_SAVER_KEY: &str = "dataSaver";
const LOCKED_CHAPTERS_KEY: &str = "lockedChapters";
const TOKEN_KEY: &str = "login";
const CODE_VERIFIER_KEY: &str = "login.codeVerifier";

pub fn get_languages() -> Result<Vec<String>> {
	defaults_get::<Vec<String>>(LANGUAGES_KEY)
		.map(|langs| {
			langs
				.into_iter()
				.map(|lang| match lang.as_str() {
					"zh-Hans" => "zh".into(),
					"zh-Hant" => "zh-hk".into(),
					"fil" => "tl".into(),
					"pt-BR" => "pt-br".into(),
					"es-419" => "es-la".into(),
					_ => lang,
				})
				.collect()
		})
		.ok_or(AidokuError::message("Unable to fetch languages"))
}

pub fn get_languages_with_key(key: &str) -> Result<String> {
	Ok(get_languages()?
		.iter()
		.fold(String::new(), |mut output, lang| {
			let _ = write!(output, "&{key}[]={lang}");
			output
		}))
}

#[derive(PartialEq, Eq)]
pub enum TitlePreference {
	Primary,
	SelectedLanguage,
	English,
	Romaji,
	Japanese,
}

pub fn get_title_preference() -> TitlePreference {
	match defaults_get::<String>(TITLE_PREFERENCE_KEY)
		.as_deref()
		.unwrap_or_default()
	{
		"" => TitlePreference::Primary,
		"select" => TitlePreference::SelectedLanguage,
		"en" => TitlePreference::English,
		"ro" => TitlePreference::Romaji,
		"ja" => TitlePreference::Japanese,
		_ => TitlePreference::Primary,
	}
}

pub fn get_content_ratings() -> Result<String> {
	Ok(defaults_get::<Vec<String>>(CONTENT_RATING_KEY)
		.ok_or(AidokuError::message(
			"Unable to fetch default content ratings",
		))?
		.iter()
		.fold(String::new(), |mut output, value| {
			let _ = write!(output, "&contentRating[]={value}");
			output
		}))
}

pub fn get_content_ratings_list() -> Result<Vec<String>> {
	defaults_get::<Vec<String>>(CONTENT_RATING_KEY).ok_or(AidokuError::message(
		"Unable to fetch default content ratings",
	))
}

pub fn get_blocked_uuids() -> Result<String> {
	Ok(defaults_get::<Vec<String>>(BLOCKED_UUIDS_KEY)
		.unwrap_or_default()
		.iter()
		.fold(String::new(), |mut output, value| {
			let _ = write!(
				output,
				"&excludedGroups[]={value}&excludedUploaders[]={value}"
			);
			output
		}))
}

pub fn get_force_port() -> bool {
	defaults_get::<bool>(FORCE_PORT_KEY).unwrap_or(false)
}

pub fn get_data_saver() -> bool {
	defaults_get::<bool>(DATA_SAVER_KEY).unwrap_or(false)
}

pub fn get_locked_chapters() -> bool {
	defaults_get::<bool>(LOCKED_CHAPTERS_KEY).unwrap_or(false)
}

pub fn get_cover_quality() -> String {
	defaults_get::<String>(COVER_QUALITY_KEY).unwrap_or_default()
}

pub fn is_logged_in() -> bool {
	defaults_get_json::<TokenResponse>(TOKEN_KEY).is_ok()
}

pub fn get_token() -> Result<TokenResponse> {
	defaults_get_json::<TokenResponse>(TOKEN_KEY).map_err(|_| AidokuError::message("Not logged in"))
}

pub fn set_token(token: &str) {
	defaults_set(TOKEN_KEY, DefaultValue::String(String::from(token)));
}

pub fn clear_token() {
	defaults_set(TOKEN_KEY, DefaultValue::Null);
}

pub fn get_code_verifier() -> Option<String> {
	defaults_get::<String>(CODE_VERIFIER_KEY)
}
