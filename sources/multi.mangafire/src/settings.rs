use aidoku::{
	alloc::{string::String, vec::Vec},
	imports::{defaults::defaults_get, error::AidokuError},
	Result,
};

// settings keys
const LANGUAGES_KEY: &str = "languages";

pub fn get_languages() -> Result<Vec<String>> {
	defaults_get::<Vec<String>>(LANGUAGES_KEY)
		.map(|langs| {
			langs
				.into_iter()
				.map(|lang| match lang.as_str() {
					"pt-BR" => "pt-br".into(),
					"es-419" => "es-la".into(),
					_ => lang,
				})
				.collect()
		})
		.ok_or(AidokuError::message("Unable to fetch languages"))
}
