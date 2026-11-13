#![no_std]
use aidoku::{HomeLayout, Result, Source, imports::html::Document, prelude::*};
use iken::{Iken, Impl, Params};

mod helpers;
mod models;

const BASE_URL: &str = "https://hivetoons.org";
const API_URL: &str = "https://api.hivetoons.org";

struct HiveScans;

impl Impl for HiveScans {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
			..Default::default()
		}
	}

	fn parse_home(&self, params: &Params, html: &Document) -> Result<HomeLayout> {
		helpers::parse_home(html, params)
	}
}

register_source!(Iken<HiveScans>, Home, DeepLinkHandler, ImageRequestProvider);
