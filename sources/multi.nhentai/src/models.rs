use crate::settings::{TitlePreference, get_title_preference};
use aidoku::{
	ContentRating, Manga, MangaStatus, UpdateStrategy, Viewer,
	alloc::{
		Vec,
		string::{String, ToString},
	},
	prelude::*,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiTag {
	pub id: i32,
	pub name: String,
	pub count: i32,
	pub r#type: String,
	pub url: String,
	pub slug: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiCover {
	pub path: String,
	pub width: i32,
	pub height: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiPageInfo {
	pub number: i32,
	pub path: String,
	pub width: i32,
	pub height: i32,
	pub thumbnail: String,
	pub thumbnail_width: i32,
	pub thumbnail_height: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiGalleryListItem {
	pub id: i32,
	pub media_id: String,
	pub thumbnail: String,
	pub thumbnail_width: i32,
	pub thumbnail_height: i32,
	pub english_title: String,
	pub japanese_title: Option<String>,
	pub tag_ids: Vec<i32>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiSearchResponse {
	pub result: Vec<NHentaiGalleryListItem>,
	pub num_pages: i32,
	pub per_page: i32,
	pub total: Option<i32>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiGallery {
	pub id: i32,
	pub media_id: String,
	pub title: NHentaiTitle,
	pub cover: NHentaiCover,
	pub thumbnail: NHentaiCover,
	pub scanlator: String,
	pub upload_date: i64,
	pub tags: Vec<NHentaiTag>,
	pub num_pages: i32,
	pub num_favorites: i32,
	pub pages: Vec<NHentaiPageInfo>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiTitle {
	pub english: String,
	pub japanese: Option<String>,
	pub pretty: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NHentaiGalleryPagesResponse {
	pub gallery_id: i32,
	pub media_id: String,
	pub num_pages: i32,
	pub pages: Vec<NHentaiPageInfo>,
}

impl NHentaiGallery {
	pub fn id_str(&self) -> String {
		self.id.to_string()
	}
}

fn make_image_url(path: &str, is_cover: bool) -> String {
	if path.starts_with("http://") || path.starts_with("https://") {
		return path.to_string();
	}

	let host = if is_cover {
		"https://t.nhentai.net"
	} else {
		"https://i.nhentai.net"
	};
	if path.starts_with('/') {
		format!("{}{}", host, path)
	} else {
		format!("{}/{}", host, path)
	}
}

impl From<NHentaiGalleryListItem> for Manga {
	fn from(value: NHentaiGalleryListItem) -> Self {
		let title_preference = get_title_preference();
		let title = match title_preference {
			TitlePreference::Japanese => match value.japanese_title {
				Some(jpn) if !jpn.is_empty() => jpn,
				_ => value.english_title,
			},
			TitlePreference::English => {
				if !value.english_title.is_empty() {
					value.english_title
				} else if let Some(jpn) = value.japanese_title {
					jpn
				} else {
					format!("#{}", value.id)
				}
			}
		};

		Manga {
			key: value.id.to_string(),
			title,
			cover: Some(make_image_url(&value.thumbnail, true)),
			description: None,
			url: Some(format!("https://nhentai.net/g/{}", value.id)),
			status: MangaStatus::Completed,
			content_rating: ContentRating::NSFW,
			viewer: Viewer::RightToLeft,
			update_strategy: UpdateStrategy::Never,
			..Default::default()
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
			info_parts.push(format!("#{}", value.id));
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
			key: value.id.to_string(),
			title,
			cover: Some(make_image_url(&value.cover.path, true)),
			description: Some(description),
			authors: Some(combined_authors),
			artists: Some(artists),
			url: Some(format!("https://nhentai.net/g/{}", value.id)),
			tags: Some(tags),
			status: MangaStatus::Completed,
			content_rating: ContentRating::NSFW,
			viewer,
			update_strategy: UpdateStrategy::Never,
			..Default::default()
		}
	}
}
