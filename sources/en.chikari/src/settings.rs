use aidoku::alloc::{String, Vec};
use aidoku::imports::defaults::defaults_get;

const HIDE_WATERMARK_KEY: &str = "hideWatermark";
const HIDE_MTL_KEY: &str = "hideMtl";
const TYPES_KEY: &str = "types";
const RATINGS_KEY: &str = "ratings";
const STATUS_KEY: &str = "status";
const HIDDEN_GENRES_KEY: &str = "hiddenGenres";

const KNOWN_TYPES: &[&str] = &["manga", "manhwa", "manhua", "oel", "novel"];
const KNOWN_RATINGS: &[&str] = &["safe", "suggestive", "erotica", "pornographic"];
const KNOWN_STATUS: &[&str] = &["releasing", "completed", "hiatus", "cancelled", "upcoming"];

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}

pub fn hide_mtl() -> bool {
	defaults_get::<bool>(HIDE_MTL_KEY).unwrap_or(false)
}

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn type_filters() -> Vec<&'static str> {
	known_values(
		defaults_get::<Vec<String>>(TYPES_KEY)
			.as_deref()
			.unwrap_or_default(),
		KNOWN_TYPES,
	)
}

pub fn rating_filters() -> Vec<&'static str> {
	known_values(
		defaults_get::<Vec<String>>(RATINGS_KEY)
			.as_deref()
			.unwrap_or_default(),
		KNOWN_RATINGS,
	)
}

pub fn status_filters() -> Vec<&'static str> {
	known_values(
		defaults_get::<Vec<String>>(STATUS_KEY)
			.as_deref()
			.unwrap_or_default(),
		KNOWN_STATUS,
	)
}

fn known_values(values: &[String], known: &[&'static str]) -> Vec<&'static str> {
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
			known_values(&["novel".into(), "manga".into()], KNOWN_TYPES),
			vec!["novel", "manga"]
		);
		assert_eq!(
			known_values(&["Erotica".into(), "bogus".into()], KNOWN_RATINGS),
			vec!["erotica"]
		);
		assert_eq!(known_values(&[], KNOWN_TYPES), Vec::<&str>::new());
	}
}
