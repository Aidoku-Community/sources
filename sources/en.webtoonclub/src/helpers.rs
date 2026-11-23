use crate::models::ReaderImage;
use aidoku::{Page, Result, alloc::Vec};

pub fn parse_page_list(body: &str) -> Result<Vec<Page>> {
	let mut images: Vec<ReaderImage> = toonz::reader_images(body)?;
	// Stable, so missing or equal positions keep the response order.
	images.sort_by_key(|image| image.sort_order);
	images.into_iter().map(Page::try_from).collect()
}
