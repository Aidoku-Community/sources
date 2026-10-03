use aidoku::{
	Manga, MangaStatus, Viewer,
	alloc::{String, Vec, format, string::ToString},
};
use serde::{Deserialize, Deserializer, Serialize};

/// LANraragi versions differ on the `pinned` field type: some return
/// `"0"`/`"1"` strings, others return integers. Accept both.
#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrI32 {
	String(String),
	Int(i32),
}

fn deserialize_pinned<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
	D: Deserializer<'de>,
{
	match StringOrI32::deserialize(deserializer)? {
		StringOrI32::String(value) => value.trim().parse().map_err(serde::de::Error::custom),
		StringOrI32::Int(value) => Ok(value),
	}
}

/// Since v0.9.80, `isnew` is emitted as a JSON boolean instead of the
/// historical `"true"`/`"false"` string (see LANraragi#1664). Accept both.
#[derive(Deserialize)]
#[serde(untagged)]
enum BoolOrString {
	Bool(bool),
	String(String),
}

fn deserialize_isnew<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
	D: Deserializer<'de>,
{
	match BoolOrString::deserialize(deserializer)? {
		BoolOrString::Bool(value) => Ok(value),
		BoolOrString::String(value) => value.trim().parse().map_err(serde::de::Error::custom),
	}
}

#[derive(Debug, Deserialize)]
pub struct ArchiveMetadata {
	pub pages: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
	pub data: Vec<Archive>,
	#[serde(default)]
	pub records_filtered: i32,
	pub records_total: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Archive {
	pub arcid: String,
	pub title: String,
	pub tags: String,
	#[serde(deserialize_with = "deserialize_isnew", default)]
	pub isnew: bool,
	pub progress: Option<i32>,
	pub lastreadtime: Option<i64>,
	pub pagecount: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Category {
	pub id: String,
	pub name: String,
	#[serde(deserialize_with = "deserialize_pinned")]
	pub pinned: i32,
	pub search: String,
	pub archives: Vec<String>,
}

impl Archive {
	pub fn into_manga(self, base_url: &str) -> Manga {
		let url = format!("{}/reader?id={}", base_url, self.arcid);
		// Remove date_added and URL tags from display tags
		let tags: Vec<String> = self
			.tags
			.split(',')
			.filter_map(|tag| {
				let tag = tag.trim();
				if tag.starts_with("date_added:") || tag.contains("://") {
					None
				} else {
					Some(tag.to_string())
				}
			})
			.collect();

		let viewer = if tags
			.iter()
			.any(|tag| tag.to_lowercase().contains("webtoon"))
		{
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};

		let arcid = self.arcid.clone();
		Manga {
			key: self.arcid,
			title: self.title,
			cover: Some(format!("{}/api/archives/{}/thumbnail", base_url, arcid)),
			url: Some(url),
			tags: Some(tags),
			status: MangaStatus::Unknown,
			update_strategy: aidoku::UpdateStrategy::Never,
			viewer,
			..Default::default()
		}
	}
}
