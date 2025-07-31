use aidoku::alloc::string::String;
use serde::Deserialize;

#[derive(Default, Deserialize, Debug, Clone)]
pub struct LibGroupCover {
	pub default: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct LibGroupAgeRestriction {
	pub label: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct LibGroupMediaType {
	pub label: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct LibGroupStatus {
	pub label: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct LibGroupTag {
	pub name: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct LibGroupMeta {
	pub has_next_page: Option<bool>,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct LibGroupTeam {
	pub name: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct LibGroupRestrictedView {
	pub is_open: bool,
}
