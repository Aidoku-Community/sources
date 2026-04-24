use crate::BASE_URL;
use aidoku::{
	Page, Result,
	alloc::{String, Vec, string::ToString as _},
	error,
	imports::{net::Request, std::current_date},
	prelude::*,
};

pub struct PageList;

impl PageList {
	pub fn get_pages(manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
		let request_id = (current_date() * 1000).to_string();
		let url = format!(
			"{}/v2.0/apis/manga/reading?code={}&cid={}&v=v4.203411&_t={}",
			BASE_URL, manga_id, chapter_id, request_id
		);
		let json: serde_json::Value = Request::get(url.clone())?
			.header(
				"Referer",
				&format!("{}/mangaread/{}/{}", BASE_URL, manga_id, chapter_id),
			)
			.header("Origin", BASE_URL)
			.header("X-Requested-With", "XMLHttpRequest")
			.header("X-Requested-Id", &request_id)
			.header("Accept", "application/json")
			.send()?
			.get_json()?;
		let data = json
			.as_object()
			.ok_or_else(|| error!("Expected JSON object"))?;
		let data = data
			.get("data")
			.and_then(|v| v.as_object())
			.ok_or_else(|| error!("Expected data object"))?;
		// `scans` can be either an array or a JSON string
		let list: Vec<serde_json::Value> = match data.get("scans") {
			Some(v) => {
				if let Some(arr) = v.as_array() {
					arr.clone()
				} else if let Some(s) = v.as_str() {
					let parsed: serde_json::Value = serde_json::from_str(s)
						.map_err(|_| error!("Failed to parse scans JSON string"))?;
					parsed
						.as_array()
						.ok_or_else(|| error!("Expected scans array after parsing"))?
						.clone()
				} else {
					bail!("Expected scans array or JSON string");
				}
			}
			None => bail!("Expected scans array"),
		};
		let mut pages: Vec<Page> = Vec::new();

		for item in list.iter() {
			let item = match item.as_object() {
				Some(item) => item,
				None => continue,
			};

			// Skip images from next chapter (n == 1)
			let n = item.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
			if n != 0 {
				continue;
			}

			let mut url = item
				.get("url")
				.and_then(|v| v.as_str())
				.unwrap_or_default()
				.to_string();

			let width = item.get("width").and_then(|v| v.as_i64()).unwrap_or(0);
			let height = item.get("height").and_then(|v| v.as_i64()).unwrap_or(0);
			if (width > 16383 || height > 16383)
				&& let Some(stripped) = url.split("?q=").next()
			{
				url = stripped.to_string();
			}

			pages.push(Page {
				content: aidoku::PageContent::url(url),
				..Default::default()
			});
		}

		Ok(pages)
	}
}
