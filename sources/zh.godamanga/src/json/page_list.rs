use crate::{API_URL, BASE_URL, IMG_URL};
use aidoku::{
	alloc::{String, Vec},
	error,
	imports::net::Request,
	prelude::format,
	Page, Result,
};

pub struct PageList;

impl PageList {
	pub fn get_pages(manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
		let ids = manga_id.split("/").collect::<Vec<&str>>();
		let url = format!(
			"{}/api/chapter/getinfo?m={}&c={}",
			API_URL,
			ids[1],
			chapter_id.clone()
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
		let list = images
			.get("images")
			.and_then(|v| v.as_array())
			.ok_or_else(|| error!("Expected images array"))?;
		let mut pages: Vec<Page> = Vec::new();

		for item in list.iter() {
			let item = match item.as_object() {
				Some(item) => item,
				None => continue,
			};
			let url = match item.get("url").and_then(|v| v.as_str()) {
				Some(url) => format!("{}{}", IMG_URL, url),
				None => continue,
			};
			pages.push(Page {
				content: aidoku::PageContent::Url(url, None),
				..Default::default()
			});
		}

		Ok(pages)
	}
}
