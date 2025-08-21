use serde::Deserialize;

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupUser {
	pub id: i32,
}
