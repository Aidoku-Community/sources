use aidoku::{
	alloc::{format, String, Vec},
	imports::std::parse_date,
	Chapter,
};
use serde::Deserialize;

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct GigaEpisode {
	pub readable_product: GigaReadableProduct,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct GigaReadableProduct {
	pub page_structure: GigaPageStructure,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct GigaPageStructure {
	pub pages: Vec<GigaPage>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct GigaPage {
	pub src: Option<String>,
	pub r#type: Option<String>,
	pub width: Option<i32>,
	pub height: Option<i32>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct GigaReadMoreResponse {
	pub html: String,
	pub next_url: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct GigaPaginationReadableProduct {
	pub display_open_at: Option<String>,
	pub readable_product_id: Option<String>,
	pub status: Option<GigaPaginationReadableProductStatus>,
	pub thumbnail_uri: Option<String>,
	pub title: Option<String>,
	pub viewer_uri: Option<String>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct GigaPaginationReadableProductStatus {
	pub label: Option<String>, // is_free, is_rentable, is_purchasable, unpublished, has_rented
}

impl From<GigaPaginationReadableProduct> for Chapter {
	fn from(val: GigaPaginationReadableProduct) -> Self {
		let chapter_number = val
			.title
			.clone()
			.and_then(crate::parser::parse_chapter_number);
		Chapter {
			key: format!("/episode/{}", val.readable_product_id.unwrap_or_default()),
			title: val.title,
			chapter_number,
			date_uploaded: val
				.display_open_at
				.and_then(|str| parse_date(str, "yyyy-MM-dd'T'HH:mm:ss'Z'")),
			url: val.viewer_uri,
			thumbnail: val.thumbnail_uri,
			locked: val
				.status
				.and_then(|status| status.label)
				.map(|label| label != "is_free" && label != "has_rented")
				.unwrap_or_default(),
			..Default::default()
		}
	}
}
