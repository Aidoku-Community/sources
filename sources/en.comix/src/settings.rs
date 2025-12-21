use aidoku::{alloc::string::String, imports::defaults::defaults_get};

// settings keys
const THUMBNAIL_QUALITY_KEY: &str = "thumbnailQuality";
const NSFW_KEY: &str = "NSFW_PREF";
const DEDUPED_CHAPTER_KEY: &str = "dedupedChapter";

// pub fn get_languages() -> Result<Vec<Language>> {
// 	defaults_get::<Vec<String>>(LANGUAGES_KEY)
// 		.map(|langs| {
// 			langs
// 				.into_iter()
// 				.map(|lang| match lang.as_str() {
// 					"en" => Language::English,
// 					"es" => Language::Spanish,
// 					"fr" => Language::French,
// 					"id" => Language::Indonesian,
// 					"pt-BR" => Language::BrazilianPortuguese,
// 					"ru" => Language::Russian,
// 					"th" => Language::Thai,
// 					"vi" => Language::Vietnamese,
// 					"de" => Language::German,
// 					_ => Language::English,
// 				})
// 				.collect()
// 		})
// 		.ok_or(error!("Unable to fetch languages"))
// }

pub fn get_image_quality() -> String {
	defaults_get::<String>(THUMBNAIL_QUALITY_KEY).unwrap_or_default()
}

pub fn get_nsfw() -> bool {
	defaults_get::<bool>(NSFW_KEY).unwrap_or(false)
}

pub fn get_dedupchapter() -> bool {
	defaults_get::<bool>(DEDUPED_CHAPTER_KEY).unwrap_or(false)
}
