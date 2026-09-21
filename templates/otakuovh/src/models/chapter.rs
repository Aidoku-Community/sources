use aidoku::{
	Chapter, Page,
	alloc::{string::String, vec::Vec},
};
use chrono::DateTime;
use serde::Deserialize;

use crate::models::common::InkRelationPublisher;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkBranch {
	pub id: String,
	#[serde(rename = "moderationStatus")]
	pub moderation_status: String,
	pub publishers: Vec<InkRelationPublisher>,
	#[serde(rename = "bookId")]
	pub book_id: String,
	pub deleted: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkChapter {
	pub id: String,
	pub name: Option<String>,
	pub title: Option<String>,
	pub number: f32,
	pub volume: f32,
	pub pages: Option<Vec<InkPage>>,
	#[serde(rename = "branchId")]
	pub branch_id: String,
	#[serde(rename = "createdAt")]
	pub created_at: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkPage {
	pub id: String,
	pub image: String,
}

impl InkChapter {
	pub fn into_chapter(self, branches: &[InkBranch]) -> Chapter {
		Chapter {
			key: self.id,
			title: self.title,
			chapter_number: Some(self.number),
			volume_number: Some(self.volume),
			date_uploaded: DateTime::parse_from_rfc3339(&self.created_at)
				.ok()
				.map(|d| d.timestamp()),
			scanlators: branches
				.iter()
				.find(|branch| self.branch_id == branch.id)
				.map(|branch| {
					branch
						.publishers
						.iter()
						.map(|publisher| publisher.name.clone())
						.collect()
				}),
			..Default::default()
		}
	}
}

impl InkPage {
	pub fn into_page(self) -> Option<Page> {
		Some(Page {
			content: aidoku::PageContent::Url(self.image, None),
			..Default::default()
		})
	}
}
