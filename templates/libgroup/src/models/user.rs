use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupUser {
	pub id: i32,
}
