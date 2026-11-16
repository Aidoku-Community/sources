use crate::BASE_URL;
use aidoku::{Manga, Result, alloc::String, prelude::*};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Banner {
	series_title: String,
	series_url: String,
	desktop_image: String,
}

impl TryFrom<Banner> for Manga {
	type Error = aidoku::AidokuError;
	fn try_from(value: Banner) -> Result<Self> {
		let path = value
			.series_url
			.strip_prefix(BASE_URL)
			.unwrap_or(&value.series_url);
		let slug = path
			.strip_prefix("/series/")
			.filter(|s| !s.trim().is_empty() && !s.contains('/'))
			.ok_or(error!("Invalid banner series"))?;
		if value.series_title.trim().is_empty() || value.desktop_image.trim().is_empty() {
			bail!("Empty banner title or image");
		}
		Ok(Self {
			key: format!("/series/{slug}"),
			title: value.series_title.trim().into(),
			// The art is 2048px wide; the row shows it at 340
			cover: Some(iken::sized_banner(&value.desktop_image, 680)),
			..Default::default()
		})
	}
}
