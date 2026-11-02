use aidoku::imports::defaults::defaults_get;

pub fn show_locked() -> bool {
	defaults_get::<bool>("showLocked").unwrap_or(false)
}
