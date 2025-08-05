#![no_std]
use aidoku::{Source, alloc::borrow::Cow, prelude::*};
use libgroup::{Impl, LibGroup, Params};

struct MangaLib;

impl Impl for MangaLib {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			site_id: Cow::Owned(1),
		}
	}
}

register_source!(
	LibGroup<MangaLib>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	AlternateCoverProvider,
	MigrationHandler
);
