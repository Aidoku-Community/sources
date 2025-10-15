use aidoku::{
	Manga, MangaStatus, Viewer,
	alloc::{String, Vec, format, string::ToString},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ArchiveMetadata {
	pub pages: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
	pub data: Vec<Archive>,
	pub draw: i32,
	pub records_filtered: i32,
	pub records_total: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Archive {
	pub arcid: String,
	pub title: String,
	pub tags: String,
	pub isnew: String,
	pub progress: Option<i32>,
	pub lastreadtime: Option<i64>,
	pub pagecount: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Category {
	pub id: String,
	pub name: String,
	pub pinned: String,
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
