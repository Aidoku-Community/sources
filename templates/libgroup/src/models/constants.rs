use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupImageServer {
	pub id: String,
	pub label: String,
	pub url: String,
	pub site_ids: Vec<u8>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupConstantsData {
	#[serde(rename = "imageServers")]
	pub image_servers: Option<Vec<LibGroupImageServer>>,
}
