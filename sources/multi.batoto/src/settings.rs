use aidoku::{
	Result,
	alloc::{string::String, vec::Vec},
	imports::defaults::defaults_get,
	prelude::*,
};

const LANGUAGES_KEY: &str = "languages";

pub fn get_languages() -> Result<Vec<String>> {
	defaults_get::<Vec<String>>(LANGUAGES_KEY)
		.map(|langs| {
			langs
				.into_iter()
				.map(|lang| match lang.as_str() {
					"en" => "en,en_us".into(),
					"pt-BR" => "pt_br".into(),
					"es-419" => "es_419".into(),
					"zh-Hans" => "zh_hk".into(),
					"zh-Hant" => "zh_tw".into(),
					"mo" => "ro-MD".into(),
					"pt-PT" => "pt_pt".into(),
					_ => lang,
				})
				.collect()
		})
		.ok_or(error!("Unable to fetch languages"))
}
