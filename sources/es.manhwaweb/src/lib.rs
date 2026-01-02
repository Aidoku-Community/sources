#![no_std]

use aidoku::{
    prelude::*,
    Source,
};

mod helper;
mod imp;
mod models;

use imp::ManhwaWeb;

register_source!(ManhwaWeb, Home, DeepLinkHandler, ListingProvider);
