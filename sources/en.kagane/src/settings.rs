use aidoku::alloc::{String, Vec};
use aidoku::imports::defaults::defaults_get;

pub fn get_wvd_key() -> String {
	defaults_get::<String>("wvdKey").unwrap_or_default()
}

pub fn get_data_saver() -> bool {
	defaults_get::<bool>("dataSaver").unwrap_or(false)
}

pub fn get_chapter_title_mode() -> String {
	defaults_get::<String>("chapterTitleMode").unwrap_or_else(|| "optional".into())
}

pub fn get_show_edition() -> bool {
	defaults_get::<bool>("showEdition").unwrap_or(false)
}

pub fn get_show_source() -> bool {
	defaults_get::<bool>("showSource").unwrap_or(false)
}

pub fn get_default_content_ratings() -> Vec<&'static str> {
	const RATINGS: &[&str] = &["safe", "suggestive", "erotica", "pornographic"];
	const DISPLAY: &[&str] = &["Safe", "Suggestive", "Erotica", "Pornographic"];

	let max = defaults_get::<String>("contentRating").unwrap_or_else(|| "pornographic".into());
	let idx = RATINGS.iter().position(|&r| r == max.as_str()).unwrap_or(3);
	DISPLAY[..=idx].to_vec()
}
