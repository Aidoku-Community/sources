use aidoku::imports::defaults::defaults_get;

pub fn label_novels() -> bool {
	defaults_get::<bool>("labelNovels").unwrap_or(true)
}
