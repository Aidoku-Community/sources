use crate::settings::{get_title_preference, TitlePreference};
use aidoku::{
	alloc::{
		string::{String, ToString},
		Vec,
	},
	prelude::*,
	ContentRating, Manga, MangaStatus, UpdateStrategy, Viewer,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
pub struct NHentaiTag {
	pub id: i32,
	pub name: String,
	pub count: i32,
	pub r#type: String,
	pub url: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiImage {
	pub t: String,
	pub w: i32,
	pub h: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiImages {
	pub pages: Vec<NHentaiImage>,
	pub cover: NHentaiImage,
	pub thumbnail: NHentaiImage,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiGallery {
	pub id: Value,
	pub media_id: String,
	pub title: NHentaiTitle,
	pub images: NHentaiImages,
	pub tags: Vec<NHentaiTag>,
	pub num_pages: i32,
	pub num_favorites: i32,
	pub upload_date: i64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiTitle {
	pub english: String,
	pub japanese: Option<String>,
	pub pretty: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiSearchResponse {
	pub result: Vec<NHentaiGallery>,
	pub num_pages: i32,
	pub per_page: i32,
}

impl NHentaiGallery {
	pub fn id_str(&self) -> String {
		match &self.id {
			Value::String(s) => s.clone(),
			Value::Number(n) => n.to_string(),
			_ => String::new(),
		}
	}
}

impl From<NHentaiGallery> for Manga {
	fn from(value: NHentaiGallery) -> Self {
		let mut tags = Vec::new();
		let mut artists = Vec::new();
		let mut groups = Vec::new();
		let mut parodies = Vec::new();
		let mut characters = Vec::new();

		for tag in &value.tags {
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

		let description = {
			let mut info_parts = Vec::new();
			info_parts.push(format!("#{}", value.id_str()));
			if !parodies.is_empty() {
				info_parts.push(format!("Parodies: {}", parodies.join(", ")));
			}
			if !characters.is_empty() {
				info_parts.push(format!("Characters: {}", characters.join(", ")));
			}
			info_parts.push(format!("Pages: {}", value.num_pages));
			if value.num_favorites > 0 {
				info_parts.push(format!("Favorited by: {}", value.num_favorites));
			}
			info_parts.join("  \n")
		};

		let title_preference = get_title_preference();
		let title = match title_preference {
			TitlePreference::Japanese => value
				.title
				.japanese
				.as_ref()
				.filter(|s| !s.is_empty())
				.unwrap_or(&value.title.english)
				.clone(),
			TitlePreference::English => value.title.english.clone(),
		};

		let viewer = if tags.iter().any(|t| t == "webtoon") {
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};

		let combined_authors = [groups, artists.clone()].concat();

		Manga {
			key: value.id_str(),
			title,
			cover: Some(format!(
				"https://t.nhentai.net/galleries/{}/cover.{}",
				value.media_id,
				extension_from_type(&value.images.cover.t)
			)),
			description: Some(description),
			authors: Some(combined_authors),
			artists: Some(artists),
			url: Some(format!("https://nhentai.net/g/{}", value.id_str())),
			tags: Some(tags),
			status: MangaStatus::Completed,
			content_rating: ContentRating::NSFW,
			viewer,
			update_strategy: UpdateStrategy::Never,
			..Default::default()
		}
	}
}
