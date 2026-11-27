use aidoku::imports::defaults::defaults_get;

const ADULT_KEY: &str = "isAdult";

/// Whether the reader confirmed they're 18 or older, like the site's age check.
pub fn is_adult() -> bool {
	defaults_get::<bool>(ADULT_KEY).unwrap_or(true)
}
