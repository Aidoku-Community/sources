use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

pub fn excluded_tags() -> Vec<String> {
	defaults_get("excludedTags").unwrap_or_default()
}

pub fn excluded_types() -> Vec<String> {
	defaults_get("excludedTypes").unwrap_or_default()
}

pub fn label_novels() -> bool {
	defaults_get("labelNovels").unwrap_or(true)
}
