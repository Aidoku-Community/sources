use aidoku::imports::defaults::defaults_get;

// const LANGUAGES_KEY: &str = "languages";
const HIDE_NSFW_KEY: &str = "hideNSFW";
const DEDUPED_CHAPTER_KEY: &str = "dedupedChapter";
const USE_VIEW_WEB_WORKAROUND_KEY: &str = "useWebViewFetch";

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

pub fn use_view_web_worker() -> bool {
	defaults_get::<bool>(USE_VIEW_WEB_WORKAROUND_KEY).unwrap_or(false)
}
