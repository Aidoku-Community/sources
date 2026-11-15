#![no_std]
use aidoku::{Source, prelude::*};
use mangataro::{Impl, MangaTaro, Params, Section::*};

const BASE_URL: &str = "https://roliascan.com";

struct RoliaScan;

impl Impl for RoliaScan {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			// No Recently Added: its grid repeats Latest Updates
			sections: &[
				Featured,
				Ranking,
				PopularChapters,
				Updates,
				Followed,
				Popular,
				Novels,
				HighScore,
			],
		}
	}
}

register_source!(
	MangaTaro<RoliaScan>,
	Home,
	DynamicFilters,
	DynamicSettings,
	DeepLinkHandler,
	ImageRequestProvider
);
