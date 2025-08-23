use aidoku::alloc::string::String;

use crate::settings::{get_api_url, get_base_url, get_cover_quality_url};

#[derive(Clone)]
pub struct Context {
	pub api_url: String,
	pub base_url: String,
	pub site_id: u8,
	pub cover_quality: String,
}

impl Context {
	pub fn new(site_id: u8) -> Self {
		Self {
			api_url: get_api_url(),
			base_url: get_base_url(),
			site_id,
			cover_quality: get_cover_quality_url(),
		}
	}

	pub fn from_params(params: &super::Params) -> Self {
		Self::new(*params.site_id)
	}
}
