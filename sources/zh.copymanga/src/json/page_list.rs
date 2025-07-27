use aidoku::{
	AidokuError, Page, PageContent, Result,
	alloc::{String, format},
	error,
	imports::defaults::defaults_get,
	serde::Deserialize,
};

#[derive(Deserialize)]
pub struct Item {
	url: String,
}

impl TryFrom<Item> for Page {
	type Error = AidokuError;

	fn try_from(page_item: Item) -> Result<Self> {
		let quality = defaults_get_string("image.quality")?;
		let format = defaults_get_string("image.format")?;
		let image = page_item
			.url
			.rsplitn(3, '.')
			.last()
			.ok_or_else(|| error!("Character `.` not found in URL: `{}`", page_item.url))?;
		let url = format!("{image}.{quality}.{format}");
		let content = PageContent::Url(url, None);
		Ok(Self {
			content,
			..Default::default()
		})
	}
}

fn defaults_get_string(key: &str) -> Result<String> {
	defaults_get(key)
		.ok_or_else(|| error!("Default does not exist or is not a string or number value"))
}
