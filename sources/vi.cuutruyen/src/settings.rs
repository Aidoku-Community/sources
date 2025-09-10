// a source made by @c0ntens
use aidoku::{alloc::string::String, imports::defaults::defaults_get};

pub fn get_url() -> String {
    defaults_get::<String>("url").unwrap_or_default()
}