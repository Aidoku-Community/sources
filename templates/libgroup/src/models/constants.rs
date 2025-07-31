use aidoku::alloc::{Vec, string::String};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize, Debug, Clone)]
pub struct LibGroupImageServer {
	pub id: String,
	pub label: String,
	pub url: String,
	pub site_ids: Vec<i32>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone)]
pub struct LibGroupConstantsData {
	#[serde(rename = "imageServers")]
	pub image_servers: Option<Vec<LibGroupImageServer>>,
}
