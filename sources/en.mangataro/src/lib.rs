#![no_std]
use aidoku::{Source, prelude::*};
use mangataro::{Impl, MangaTaro, Params, Section::*};

const BASE_URL: &str = "https://mangataro.org";

struct MangaTaroSource;

impl Impl for MangaTaroSource {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			sections: &[
				Featured,
				Recent,
				Updates,
				Popular,
				Ranking,
				PopularChapters,
				Followed,
				LatestNovels,
				HighScore,
			],
		}
	}
}

register_source!(
	MangaTaro<MangaTaroSource>,
	Home,
	DynamicFilters,
	DynamicSettings,
	DeepLinkHandler,
	ImageRequestProvider
);
