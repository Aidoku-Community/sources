use aidoku::alloc::{String, Vec};
use aidoku::imports::defaults::defaults_get;

const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const TYPES_KEY: &str = "types";
const HIDDEN_GENRES_KEY: &str = "hiddenGenres";

const KNOWN_TYPES: &[&str] = &["manga", "manhwa", "manhua", "oel", "novel"];

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn type_filters() -> Vec<&'static str> {
	valid_types(defaults_get::<Vec<String>>(TYPES_KEY).unwrap_or_default())
}

fn valid_types(values: Vec<String>) -> Vec<&'static str> {
	values
		.iter()
		.filter_map(|value| {
			KNOWN_TYPES
				.iter()
				.copied()
				.find(|known| value.eq_ignore_ascii_case(known))
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::alloc::vec;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn type_setting_keeps_known_types() {
		assert_eq!(
			valid_types(vec!["novel".into(), "manga".into()]),
			vec!["novel", "manga"]
		);
		assert_eq!(valid_types(vec!["bogus".into()]), Vec::<&str>::new());
		assert_eq!(valid_types(Vec::new()), Vec::<&str>::new());
	}
}
