#![no_std]
use aidoku::{Source, prelude::*};
use heancms::{HeanCms, Impl, Params};

const BASE_URL: &str = "https://luacomic.org";
const API_URL: &str = "https://api.luacomic.org";

struct LuaScans;

impl Impl for LuaScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
			latest_sort_ascending: true,
		}
	}
}

register_source!(HeanCms<LuaScans>, DynamicFilters, DeepLinkHandler);
