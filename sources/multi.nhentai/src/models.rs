use aidoku::{
	alloc::{string::String, string::ToString, Vec},
	prelude::*,
	ContentRating, Manga, MangaStatus, Viewer,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::settings::{get_title_preference, TitlePreference};

pub fn extension_from_type(t: &str) -> &str {
	match t {
		"j" => "jpg",
		"p" => "png",
		"w" => "webp",
		"g" => "gif",
		_ => "jpg", // default to jpg
	}
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NhentaiTag {
	pub id: i32,
	pub name: String,
	pub count: i32,
	pub r#type: String,
	pub url: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NhentaiImage {
	pub t: String,
	pub w: i32,
	pub h: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NhentaiImages {
	pub pages: Vec<NhentaiImage>,
	pub cover: NhentaiImage,
	pub thumbnail: NhentaiImage,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NhentaiGallery {
	pub id: Value,
	pub media_id: String,
	pub title: NhentaiTitle,
	pub images: NhentaiImages,
	pub tags: Vec<NhentaiTag>,
	pub num_pages: i32,
	pub num_favorites: i32,
	pub upload_date: i64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NhentaiTitle {
	pub english: String,
	pub japanese: Option<String>,
	pub pretty: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NhentaiSearchResponse {
	pub result: Vec<NhentaiGallery>,
	pub num_pages: i32,
	pub per_page: i32,
}

impl NhentaiGallery {
	pub fn id_str(&self) -> String {
		match &self.id {
			Value::String(s) => s.clone(),
			Value::Number(n) => n.to_string(),
			_ => String::new(),
		}
	}

	pub fn into_manga(self) -> Manga {
		let mut tags = Vec::new();
		let mut artists = Vec::new();
		let mut groups = Vec::new();
		let mut parodies = Vec::new();
		let mut characters = Vec::new();

		for tag in &self.tags {
			match tag.r#type.as_str() {
				"tag" => tags.push((tag.name.clone(), tag.count)),
				"artist" => artists.push((tag.name.clone(), tag.count)),
				"group" => groups.push((tag.name.clone(), tag.count)),
				"parody" => {
					if tag.name != "original" && tag.name != "various" {
						parodies.push((tag.name.clone(), tag.count));
					}
				}
				"character" => characters.push((tag.name.clone(), tag.count)),
				_ => {}
			}
		}

		// Sort by count descending
		tags.sort_by(|a, b| b.1.cmp(&a.1));
		artists.sort_by(|a, b| b.1.cmp(&a.1));
		groups.sort_by(|a, b| b.1.cmp(&a.1));
		parodies.sort_by(|a, b| b.1.cmp(&a.1));
		characters.sort_by(|a, b| b.1.cmp(&a.1));

		// Extract names
		let tags = tags.into_iter().map(|(name, _)| name).collect::<Vec<_>>();
		let groups = groups.into_iter().map(|(name, _)| name).collect::<Vec<_>>();
		let artists = artists
			.into_iter()
			.map(|(name, _)| name)
			.collect::<Vec<_>>();
		let parodies = parodies
			.into_iter()
			.map(|(name, _)| name)
			.collect::<Vec<_>>();
		let characters = characters
			.into_iter()
			.map(|(name, _)| name)
			.collect::<Vec<_>>();

		let mut info_parts = Vec::new();

		if !parodies.is_empty() {
			info_parts.push(format!("Parodies: {}", parodies.join(", ")));
		}
		if !characters.is_empty() {
			info_parts.push(format!("Characters: {}", characters.join(", ")));
		}

		let mut description = format!("#{}", self.id_str());
		if !info_parts.is_empty() {
			description.push_str("  \n");
			description.push_str(&info_parts.join("  \n"));
		}

		let title_preference = get_title_preference();
		let title = match title_preference {
			TitlePreference::Japanese => self
				.title
				.japanese
				.as_ref()
				.filter(|s| !s.is_empty())
				.unwrap_or(&self.title.english)
				.clone(),
			TitlePreference::English => self.title.english.clone(),
		};

		let viewer = if tags.iter().any(|t| t == "webtoon") {
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};

		let mut combined_authors = groups.clone();
		combined_authors.extend(artists.clone());

		Manga {
			key: self.id_str(),
			title,
			cover: Some(format!(
				"https://t.nhentai.net/galleries/{}/cover.{}",
				self.media_id,
				extension_from_type(&self.images.cover.t)
			)),
			description: Some(description),
			url: Some(format!("https://nhentai.net/g/{}", self.id_str())),
			authors: Some(combined_authors),
			artists: Some(artists),
			tags: Some(tags),
			content_rating: ContentRating::NSFW,
			status: MangaStatus::Completed,
			viewer,
			..Default::default()
		}
	}
}
