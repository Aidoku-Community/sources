use aidoku::imports::defaults::defaults_get;

pub fn show_locked() -> bool {
	defaults_get::<bool>("showLocked").unwrap_or(false)
}

pub fn label_novels() -> bool {
	defaults_get::<bool>("labelNovels").unwrap_or(true)
}
