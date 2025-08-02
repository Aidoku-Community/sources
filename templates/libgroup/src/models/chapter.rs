use aidoku::{
	Chapter, Page, PageContent,
	alloc::{
		Vec,
		string::{String, ToString},
	},
	prelude::*,
};
use chrono::DateTime;
use serde::Deserialize;

use crate::cdn::get_selected_image_server_url;

use super::common::{LibGroupRestrictedView, LibGroupTeam};

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupChapterBranch {
	pub id: i32,
	pub branch_id: Option<i32>,
	pub created_at: String,
	pub teams: Vec<LibGroupTeam>,
	pub restricted_view: Option<LibGroupRestrictedView>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupChapterListItem {
	pub volume: String,
	pub number: String,
	pub name: Option<String>,
	pub branches: Vec<LibGroupChapterBranch>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupPage {
	pub url: String,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupChapter {
	pub pages: Vec<LibGroupPage>,
}

impl LibGroupChapterListItem {
	pub fn into_chapters(self, base_url: &str, slug_url: &str) -> Vec<Chapter> {
		self.branches
			.into_iter()
			.map(|branch| {
				let chapter_number = self.number.parse::<f32>().ok();
				let volume_number = self.volume.parse::<f32>().ok();

				let scanlators: Vec<String> =
					branch.teams.iter().map(|team| team.name.clone()).collect();

				let locked = branch
					.restricted_view
					.as_ref()
					.map(|rv| !rv.is_open)
					.unwrap_or(false);

				Chapter {
					key: branch.id.to_string(),
					title: self.name.clone(),
					chapter_number,
					volume_number,
					date_uploaded: DateTime::parse_from_rfc3339(&branch.created_at)
						.ok()
						.map(|d| d.timestamp()),
					scanlators: Some(scanlators),
					url: Some(format!(
						"{}/ru/{}/read/v{}/c{}{}",
						base_url,
						slug_url,
						volume_number.unwrap_or_default(),
						chapter_number.unwrap_or_default(),
						branch
							.branch_id
							.map(|id| format!("?bid={id}"))
							.unwrap_or_default()
					)),
					locked,
					..Default::default()
				}
			})
			.collect()
	}

	pub fn flatten_chapters(items: Vec<Self>, base_url: &str, slug_url: &str) -> Vec<Chapter> {
		items
			.into_iter()
			.flat_map(|item| item.into_chapters(base_url, slug_url))
			.collect()
	}
}

impl LibGroupChapter {
	pub fn into_pages(self, site_id: &u8) -> Vec<Page> {
		self.pages
			.into_iter()
			.map(|page| Page {
				content: PageContent::url(format!(
					"{}{}",
					get_selected_image_server_url(site_id),
					page.url
				)),
				..Default::default()
			})
			.collect()
	}
}
