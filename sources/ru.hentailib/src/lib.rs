#![no_std]
use aidoku::{Source, alloc::borrow::Cow, prelude::*};
use libgroup::{Impl, LibGroup, Params};

struct HentaiLib;

impl Impl for HentaiLib {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			site_id: Cow::Owned(4),
		}
	}
}

register_source!(
	LibGroup<HentaiLib>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	AlternateCoverProvider,
	MigrationHandler
);
