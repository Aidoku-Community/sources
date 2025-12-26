use aidoku::{
	Chapter, HashMap, Page, PageContent,
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
};
use chrono::DateTime;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::{
	cdn::get_selected_image_server_url,
	context::Context,
	converters::{convert_html_to_markdown, convert_model_to_markdown},
	endpoints::Url,
	models::common::LibGroupModerated,
};

use super::common::{LibGroupRestrictedView, LibGroupTeam};

#[derive(Default, Deserialize, Clone)]
#[serde(default)]
pub struct LibGroupChapterBranch {
	pub id: i32,
	pub branch_id: Option<i32>,
	pub created_at: String,
	pub teams: Vec<LibGroupTeam>,
	pub user: LibGroupChapterBranchUser,
	pub restricted_view: Option<LibGroupRestrictedView>,
	pub moderation: Option<LibGroupModerated>,
}

#[derive(Default, Deserialize, Clone)]
#[serde(default)]
pub struct LibGroupChapterBranchUser {
	pub username: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum LibGroupBranchesFormat {
	Array(Vec<LibGroupChapterBranch>),
	Object(HashMap<String, LibGroupChapterBranch>),
}

impl LibGroupBranchesFormat {
	fn into_vec(self) -> Vec<LibGroupChapterBranch> {
		match self {
			LibGroupBranchesFormat::Array(vec) => vec,
			LibGroupBranchesFormat::Object(map) => map.into_values().collect(),
		}
	}
}

fn deserialize_branches<'de, D>(deserializer: D) -> Result<Vec<LibGroupChapterBranch>, D::Error>
where
	D: Deserializer<'de>,
{
	LibGroupBranchesFormat::deserialize(deserializer).map(|format| format.into_vec())
}

#[derive(Default, Deserialize, Clone)]
#[serde(default)]
pub struct LibGroupChapterListItem {
	pub volume: String,
	pub number: String,
	pub name: Option<String>,
	#[serde(deserialize_with = "deserialize_branches")]
	pub branches: Vec<LibGroupChapterBranch>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupPage {
	pub url: String,
}

#[derive(Deserialize)]
pub struct LibGroupImageChapter {
	pub pages: Vec<LibGroupPage>,
}

#[derive(Deserialize)]
pub struct LibGroupTextChapter {
	pub content: LibGroupContentType,
	pub attachments: Vec<LibGroupAttachment>,
}

pub enum LibGroupContentType {
	Html(String),
	Model(LibGroupContentModel),
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupContentModel {
	pub content: Option<Vec<LibGroupContentNode>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupContentNode {
	#[serde(rename = "type")]
	pub node_type: String,
	pub content: Option<Vec<LibGroupTextNode>>,
	pub attrs: Option<LibGroupNodeAttrs>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupTextNode {
	pub text: Option<String>,
	pub marks: Option<Vec<LibGroupMark>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupMark {
	#[serde(rename = "type")]
	pub mark_type: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupNodeAttrs {
	pub images: Option<Vec<LibGroupImageAttr>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupImageAttr {
	pub image: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct LibGroupAttachment {
	pub url: String,
	pub name: Option<String>,
	pub filename: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum LibGroupChapterData {
	// Image chapter has "pages" field
	Image(LibGroupImageChapter),
	// Text chapter has "content" field (and other text-specific fields)
	Text(LibGroupTextChapter),
}

impl Default for LibGroupContentType {
	fn default() -> Self {
		LibGroupContentType::Html(String::new())
	}
}

impl<'de> Deserialize<'de> for LibGroupContentType {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let value = Value::deserialize(deserializer)?;
		match value {
			Value::String(s) => Ok(LibGroupContentType::Html(s)),
			Value::Object(_) => {
				let model: LibGroupContentModel =
					serde_json::from_value(value).map_err(serde::de::Error::custom)?;
				Ok(LibGroupContentType::Model(model))
			}
			_ => Err(serde::de::Error::custom("Invalid content type")),
		}
	}
}

impl LibGroupChapterListItem {
	pub fn into_chapters(
		self,
		base_url: &str,
		slug_url: &str,
		user_id: &Option<i32>,
	) -> Vec<Chapter> {
		self.branches
			.into_iter()
			.map(|branch| {
				let chapter_number = self.number.parse::<f32>().ok();
				let volume_number = self.volume.parse::<f32>().ok();

				let scanlators: Vec<String> = if branch.teams.is_empty() {
					vec![branch.user.username.clone()]
				} else {
					branch.teams.iter().map(|team| team.name.clone()).collect()
				};

				let locked = branch
					.restricted_view
					.as_ref()
					.map(|rv| !rv.is_open)
					.unwrap_or(false)
					|| branch
						.moderation
						.as_ref()
						.map(|m| m.label == "На модерации")
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
					url: Some(Url::chapter_page(
						base_url,
						slug_url,
						volume_number,
						chapter_number,
						branch.branch_id,
						user_id,
					)),
					locked,
					..Default::default()
				}
			})
			.collect()
	}

	pub fn flatten_chapters(
		items: Vec<Self>,
		base_url: &str,
		slug_url: &str,
		user_id: &Option<i32>,
	) -> Vec<Chapter> {
		items
			.into_iter()
			.rev()
			.flat_map(|item| item.into_chapters(base_url, slug_url, user_id))
			.collect()
	}
}

impl LibGroupChapterData {
	pub fn into_pages(self, ctx: &Context) -> Vec<Page> {
		match self {
			LibGroupChapterData::Image(chapter) => chapter.into_pages(ctx),
			LibGroupChapterData::Text(chapter) => chapter.into_pages(ctx),
		}
	}
}

impl LibGroupImageChapter {
	pub fn into_pages(self, ctx: &Context) -> Vec<Page> {
		self.pages
			.into_iter()
			.map(|page| Page {
				content: PageContent::url(format!(
					"{}{}",
					get_selected_image_server_url(ctx),
					page.url
				)),
				..Default::default()
			})
			.collect()
	}
}

impl LibGroupTextChapter {
	pub fn into_pages(self, ctx: &Context) -> Vec<Page> {
		let markdown = match self.content {
			LibGroupContentType::Html(html) => convert_html_to_markdown(&html),
			LibGroupContentType::Model(model) => {
				convert_model_to_markdown(&model, &self.attachments, ctx)
			}
		};

		vec![Page {
			content: PageContent::text(markdown),
			..Default::default()
		}]
	}
}
