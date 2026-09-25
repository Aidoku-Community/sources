use aidoku::{
	Link, Manga,
	alloc::{borrow::ToOwned, format, string::String, vec::Vec},
};
use serde::Deserialize;

use crate::models::common::{InkLabel, InkMangaAlias, InkRelation};

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkManga {
	pub id: String,
	pub slug: String,
	#[serde(rename = "type")]
	pub content_type: String,
	#[serde(rename = "serviceName")]
	pub service_name: String,
	pub aliases: Vec<InkMangaAlias>,
	pub poster: String,
	pub name: InkMangaName,
	pub country: String,
	pub year: i32,
	pub formats: Vec<String>,
	#[serde(rename = "chaptersCount")]
	pub chapters_count: i32,
	pub status: String,
	pub description: Option<String>,
	pub relations: Option<Vec<InkRelation>>,
	pub labels: Vec<InkLabel>,
	#[serde(rename = "contentStatus")]
	pub content_status: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct InkMangaName {
	pub en: String,
	pub ru: String,
	pub original: String,
}

impl InkManga {
	fn get_relation_names(&self, relation_type: &str) -> Vec<String> {
		self.relations
			.iter()
			.flatten()
			.filter(|r| r.type_relation == relation_type)
			.filter_map(|r| r.publisher.as_ref().map(|p| p.name.to_owned()))
			.collect()
	}

	pub fn into_link(self) -> Link {
		let title = self.name.ru.clone();
		let image_url = Some(self.poster.clone());
		let subtitle = Some(format!("{} · {}", self.country, self.year));

		Link {
			title,
			image_url,
			subtitle,
			value: Some(aidoku::LinkValue::Manga(self.into_basic_manga())),
		}
	}

	pub fn into_basic_manga(self) -> Manga {
		Manga {
			key: self.id,
			title: self.name.ru,
			cover: Some(self.poster),
			..Default::default()
		}
	}

	pub fn into_detailed_manga(self, domain: String) -> Manga {
		let artists = self.get_relation_names("ARTIST");
		let authors = self.get_relation_names("AUTHOR");

		let status = match self.status.as_str() {
			"ONGOING" => aidoku::MangaStatus::Ongoing,
			"DONE" => aidoku::MangaStatus::Completed,
			_ => aidoku::MangaStatus::Unknown,
		};

		let update_strategy = match self.status.as_str() {
			"ONGOING" => aidoku::UpdateStrategy::Always,
			"DONE" => aidoku::UpdateStrategy::Never,
			_ => aidoku::UpdateStrategy::Always,
		};

		let content_rating = match self.content_status.as_str() {
			"SAFE" => aidoku::ContentRating::Safe,
			"UNSAFE" => aidoku::ContentRating::NSFW,
			_ => aidoku::ContentRating::Suggestive,
		};

		let viewer = match self.formats.first().map(|s| s.as_str()) {
			Some("WEBTOON") => aidoku::Viewer::Webtoon,
			_ => aidoku::Viewer::RightToLeft, // adjust as needed
		};

		Manga {
			key: self.id,
			title: self.name.ru,
			cover: Some(self.poster),
			description: self.description,
			status,
			artists: Some(artists),
			authors: Some(authors),
			tags: Some(self.labels.into_iter().map(|l| l.name).collect()),
			content_rating,
			update_strategy,
			viewer,
			url: Some(format!("https://{}/content/{}", domain, self.slug)),
			..Default::default()
		}
	}
}

