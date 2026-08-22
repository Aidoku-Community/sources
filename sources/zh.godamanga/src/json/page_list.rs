use crate::json::chapter_decoder::ChapterImageDecoder;
use crate::{API_URL, BASE_URL, IMG_URL};
use aidoku::{
	Page, Result,
	alloc::{String, Vec},
	error,
	imports::net::Request,
	prelude::*,
};

pub struct PageList;

impl PageList {
	pub fn get_pages(manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
		let ids = manga_id.split("/").collect::<Vec<&str>>();
		let url = format!(
			"{}/api/v2/chapter/getinfo?m={}&c={}",
			API_URL, ids[1], chapter_id
		);

		let json: serde_json::Value = Request::get(url.clone())?
			.header("Origin", BASE_URL)
			.header("Referer", BASE_URL)
			.send()?
			.get_json()?;
		let data = json
			.as_object()
			.ok_or_else(|| error!("Expected JSON object"))?;
		let data = data
			.get("data")
			.and_then(|v| v.as_object())
			.ok_or_else(|| error!("Expected data object"))?;
		let info = data
			.get("info")
			.and_then(|v| v.as_object())
			.ok_or_else(|| error!("Expected info object"))?;
		let images = info
			.get("images")
			.and_then(|v| v.as_object())
			.ok_or_else(|| error!("Expected images object"))?;

		let encoded = images
			.get("images")
			.and_then(|v| v.as_str())
			.ok_or_else(|| error!("Expected images string"))?;

		let decoded = ChapterImageDecoder::decode(encoded)?;
		let list = serde_json::from_str::<Vec<ImageItem>>(&decoded)
			.map_err(|_| error!("Failed to parse decoded images"))?;

		let mut pages: Vec<Page> = Vec::new();
		for item in list.iter() {
			pages.push(Page {
				content: aidoku::PageContent::Url(format!("{}{}", IMG_URL, item.url), None),
				..Default::default()
			});
		}

		Ok(pages)
	}
}

#[derive(serde::Deserialize)]
struct ImageItem {
	url: String,
}
