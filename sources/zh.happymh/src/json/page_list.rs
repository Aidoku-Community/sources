use crate::BASE_URL;
use aidoku::{
	alloc::{string::ToString as _, String, Vec},
	error,
	imports::net::Request,
	prelude::format,
	Page, Result,
};

pub struct PageList;

impl PageList {
	pub fn get_pages(_manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
		let url = format!(
			"{}/v2.0/apis/manga/reading?code={}&v=v3.1818134",
			BASE_URL, chapter_id
		);
		let json: serde_json::Value = Request::get(url.clone())?
			.header(
				"Referer",
				&format!("{}/mangaread/{}", BASE_URL, chapter_id.clone()),
			)
			.header("Origin", BASE_URL)
			.header("X-Requested-With", "XMLHttpRequest")
			.send()?
			.get_json()?;
		let data = json
			.as_object()
			.ok_or_else(|| error!("Expected JSON object"))?;
		let data = data
			.get("data")
			.and_then(|v| v.as_object())
			.ok_or_else(|| error!("Expected data object"))?;
		let list = data
			.get("scans")
			.and_then(|v| v.as_array())
			.ok_or_else(|| error!("Expected scans array"))?;
		let mut pages: Vec<Page> = Vec::new();

		for item in list.iter() {
			let item = match item.as_object() {
				Some(item) => item,
				None => continue,
			};
			let url = item
				.get("url")
				.and_then(|v| v.as_str())
				.unwrap_or_default()
				.to_string();
			pages.push(Page {
				content: aidoku::PageContent::url(url),
				..Default::default()
			});
		}

		Ok(pages)
	}
}
