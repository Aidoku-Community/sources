use aidoku::alloc::{String, Vec};
use aidoku::imports::defaults::defaults_get;

const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const TYPES_KEY: &str = "types";
const RATINGS_KEY: &str = "ratings";
const HIDDEN_GENRES_KEY: &str = "hiddenGenres";

const KNOWN_TYPES: &[&str] = &["manga", "manhwa", "manhua", "oel", "novel"];
const KNOWN_RATINGS: &[&str] = &["safe", "suggestive", "erotica", "pornographic"];

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn type_filters() -> Vec<&'static str> {
	known_values(
		defaults_get::<Vec<String>>(TYPES_KEY).unwrap_or_default(),
		KNOWN_TYPES,
	)
}

pub fn rating_filters() -> Vec<&'static str> {
	known_values(
		defaults_get::<Vec<String>>(RATINGS_KEY).unwrap_or_default(),
		KNOWN_RATINGS,
	)
}

fn known_values(values: Vec<String>, known: &[&'static str]) -> Vec<&'static str> {
	values
		.iter()
		.filter_map(|value| {
			known
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
	fn settings_keep_known_values() {
		assert_eq!(
			known_values(vec!["novel".into(), "manga".into()], KNOWN_TYPES),
			vec!["novel", "manga"]
		);
		assert_eq!(
			known_values(vec!["Erotica".into(), "bogus".into()], KNOWN_RATINGS),
			vec!["erotica"]
		);
		assert_eq!(known_values(Vec::new(), KNOWN_TYPES), Vec::<&str>::new());
	}
}
