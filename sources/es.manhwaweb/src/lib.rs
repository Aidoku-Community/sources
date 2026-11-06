#![no_std]

use aidoku::{Source, prelude::*};

mod helpers;
mod imp;
mod models;

use imp::ManhwaWeb;

register_source!(ManhwaWeb, Home, DeepLinkHandler, ListingProvider);
