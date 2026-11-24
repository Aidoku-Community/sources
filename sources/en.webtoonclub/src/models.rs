use crate::BASE_URL;
use aidoku::{AidokuError, Page, Result, alloc::String};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderImage {
	filename: String,
	#[serde(default)]
	pub sort_order: u32,
}

impl TryFrom<ReaderImage> for Page {
	type Error = AidokuError;

	fn try_from(image: ReaderImage) -> Result<Self> {
		toonz::upload_page(BASE_URL, &image.filename)
	}
}
