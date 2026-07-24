use crate::LOGIN_COOKIE_KEY;
use aidoku::{
	alloc::{string::String, vec, vec::Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_get_map, defaults_set},
};

// const LANGUAGES_KEY: &str = "languages";

const HIDE_NSFW_KEY: &str = "hideNSFW";
const DEDUPED_CHAPTER_KEY: &str = "dedupedChapter";
const SHOW_STANDALONE_VOLUME_KEY: &str = "showVolumes";

const LOGIN_KEY: &str = "login";

const DEDUPED_GROUP_KEY: &str = "deduplicateGroupList";
pub const NOTIFICATION_RESET_DEDUPED_GROUP_KEY: &str = "resetDeduplicateGroupList";

const DEFAULT_CONTENT_TYPES_KEY: &str = "contentTypes";
pub const NOTIFICATION_RESET_FILTERS_KEY: &str = "resetFilters";

/* Not in use yet, but maybe we need to do some mapping once we get enough data on how the language field works.
pub fn get_languages() -> Result<Vec<String>> {
	defaults_get::<Vec<String>>(LANGUAGES_KEY)
		.map(|languages| {
			languages
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
		.ok_or(error!("Unable to fetch languages"))
}
*/

pub fn hide_nsfw() -> bool {
	defaults_get::<bool>(HIDE_NSFW_KEY).unwrap_or(true)
}

pub fn deduped_chapter() -> bool {
	defaults_get::<bool>(DEDUPED_CHAPTER_KEY).unwrap_or(false)
}

pub fn show_standalone_volume() -> bool {
	defaults_get::<bool>(SHOW_STANDALONE_VOLUME_KEY).unwrap_or(false)
}

pub fn get_login_cookie() -> Option<String> {
	defaults_get_map(LOGIN_KEY)?.get(LOGIN_COOKIE_KEY).cloned()
}

pub fn get_deduped_group_list() -> Vec<String> {
	defaults_get::<Vec<String>>(DEDUPED_GROUP_KEY).unwrap_or(vec![])
}

pub fn reset_deduped_group_list() {
	defaults_set(DEDUPED_GROUP_KEY, DefaultValue::Null)
}

pub fn get_default_content_types() -> Option<String> {
	defaults_get::<Vec<String>>(DEFAULT_CONTENT_TYPES_KEY).map(|ids| ids.join(","))
}

pub fn reset_filters() {
	defaults_set(DEFAULT_CONTENT_TYPES_KEY, DefaultValue::Null)
}
