use aidoku::alloc::String;
use serde::Deserialize;

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupCover {
	pub thumbnail: Option<String>,
	pub default: String,
	pub md: Option<String>,
	pub orig: Option<String>,
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
pub struct LibGroupRating {
	pub average: String,
	pub votes: i32,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupStatus {
	pub label: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupModerated {
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

impl LibGroupCover {
	/// Get cover URL based on user preference with fallback logic
	pub fn get_cover_url(&self, quality: &str) -> String {
		match quality {
			"thumbnail" => {
				// thumbnail > default
				self.thumbnail.as_ref().unwrap_or(&self.default).clone()
			}
			"md" => {
				// md > default
				self.md.as_ref().unwrap_or(&self.default).clone()
			}
			"orig" => {
				// orig > md > default
				self.orig
					.as_ref()
					.or(self.md.as_ref())
					.unwrap_or(&self.default)
					.clone()
			}
			_ => {
				// default or unknown quality
				self.default.clone()
			}
		}
	}
}
