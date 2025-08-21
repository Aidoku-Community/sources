#![no_std]
use aidoku::{Source, alloc::borrow::Cow, prelude::*};
use libgroup::{Impl, LibGroup, Params};

struct SlashLib;

impl Impl for SlashLib {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			site_id: Cow::Owned(2),
		}
	}
}

register_source!(
	LibGroup<SlashLib>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	AlternateCoverProvider,
	MigrationHandler
);
