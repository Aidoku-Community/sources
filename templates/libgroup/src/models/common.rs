use aidoku::alloc::String;
use serde::Deserialize;

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupCover {
	pub default: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupAgeRestriction {
	pub label: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupMediaType {
	pub label: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupStatus {
	pub label: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupTag {
	pub name: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupMeta {
	pub has_next_page: Option<bool>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupTeam {
	pub name: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupRestrictedView {
	pub is_open: bool,
}
