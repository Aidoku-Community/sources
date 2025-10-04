use aidoku::{
	alloc::{string::String, vec::Vec},
	imports::defaults::defaults_get,
};

const TITLE_PREFERENCE_KEY: &str = "titlePreference";
const LANGUAGES_KEY: &str = "languages";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TitlePreference {
	English,
	Japanese,
}

impl Default for TitlePreference {
	fn default() -> Self {
		Self::English
	}
}

impl From<String> for TitlePreference {
	fn from(value: String) -> Self {
		match value.as_str() {
			"japanese" => Self::Japanese,
			"english" => Self::English,
			_ => Self::English,
		}
	}
}

pub fn get_title_preference() -> TitlePreference {
	defaults_get::<String>(TITLE_PREFERENCE_KEY)
		.map(TitlePreference::from)
		.unwrap_or_default()
}

pub fn get_languages() -> Vec<String> {
	defaults_get::<Vec<String>>(LANGUAGES_KEY)
		.unwrap_or_default()
		.into_iter()
		.map(|lang| match lang.as_str() {
			"en" => "english".into(),
			"ja" => "japanese".into(),
			"zh" => "chinese".into(),
			_ => lang,
		})
		.collect()
}
