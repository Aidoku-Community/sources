#![no_std]
use aidoku::{Source, alloc::borrow::Cow, prelude::*};
use libgroup::{Impl, LibGroup, Params};

struct RanobeLib;

impl Impl for RanobeLib {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			site_id: Cow::Owned(3),
		}
	}
}

register_source!(
	LibGroup<RanobeLib>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	AlternateCoverProvider,
	MigrationHandler
);
