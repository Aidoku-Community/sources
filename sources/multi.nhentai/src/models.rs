use crate::localization_cn::*;
use crate::settings::{self, TitlePreference};
use aidoku::{
	ContentRating, Manga, MangaStatus, UpdateStrategy, Viewer,
	alloc::{
		Vec,
		string::{String, ToString},
	},
	prelude::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn get_tag_map(lang: &str) -> Option<&'static phf::Map<&'static str, &'static str>> {
	match lang {
		"chinese" => Some(&CN_TAG),
		_ => None,
	}
}

fn get_parody_map(lang: &str) -> Option<&'static phf::Map<&'static str, &'static str>> {
	match lang {
		"chinese" => Some(&CN_PARODY),
		_ => None,
	}
}

fn get_character_map(lang: &str) -> Option<&'static phf::Map<&'static str, &'static str>> {
	match lang {
		"chinese" => Some(&CN_CHARACTER),
		_ => None,
	}
}

fn get_group_map(lang: &str) -> Option<&'static phf::Map<&'static str, &'static str>> {
	match lang {
		"chinese" => Some(&CN_GROUP),
		_ => None,
	}
}

fn get_artist_map(lang: &str) -> Option<&'static phf::Map<&'static str, &'static str>> {
	match lang {
		"chinese" => Some(&CN_ARTIST),
		_ => None,
	}
}

/// Translate a name using PHF map (O(1) lookup)
#[inline]
fn translate_name(name: &str, map: Option<&phf::Map<&'static str, &'static str>>) -> String {
	match map {
		Some(m) => {
			let lower = name.to_lowercase();
			m.get(lower.as_str())
				.map(|s| (*s).to_string())
				.unwrap_or_else(|| name.to_string())
		}
		None => name.to_string(),
	}
}

/// Reverse translate a localized tag to English (for search)
pub fn reverse_translate_tag(query: &str) -> String {
	let lang = settings::get_tag_language();
	if lang == "english" {
		return query.to_string();
	}

	let lower = query.to_lowercase();
	match lang.as_str() {
		"chinese" => CN_TAG_REVERSE
			.get(lower.as_str())
			.map(|s| (*s).to_string())
			.unwrap_or_else(|| query.to_string()),
		_ => query.to_string(),
	}
}

/// Translate an English tag to localized version (for display)
pub fn translate_tag(tag: &str, lang: &str) -> String {
	if lang == "english" {
		return tag.to_string();
	}

	let map = get_tag_map(lang);
	translate_name(tag, map)
}

pub fn extension_from_type(t: &str) -> &str {
	match t {
		"j" => "jpg",
		"p" => "png",
		"w" => "webp",
		"g" => "gif",
		_ => "jpg",
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
		let tag_lang = settings::get_tag_language();
		let metadata_lang = settings::get_metadata_language();
		let title_preference = settings::get_title_preference();

		let mut tags = Vec::new();
		let mut artists = Vec::new();
		let mut groups = Vec::new();
		let mut parodies = Vec::new();
		let mut characters = Vec::new();

		let tag_map = get_tag_map(&tag_lang);
		let artist_map = get_artist_map(&metadata_lang).or_else(|| get_artist_map(&tag_lang));
		let group_map = get_group_map(&metadata_lang).or_else(|| get_group_map(&tag_lang));
		let parody_map = get_parody_map(&metadata_lang).or_else(|| get_parody_map(&tag_lang));
		let character_map = get_character_map(&metadata_lang).or_else(|| get_character_map(&tag_lang));

		for tag in &value.tags {
			match tag.r#type.as_str() {
				"tag" => {
					let name = translate_name(&tag.name, tag_map);
					tags.push((name, tag.count));
				}
				"artist" => {
					let name = translate_name(&tag.name, artist_map);
					artists.push((name, tag.count));
				}
				"group" => {
					let name = translate_name(&tag.name, group_map);
					groups.push((name, tag.count));
				}
				"parody" => {
					if tag.name != "original" && tag.name != "various" {
						let name = translate_name(&tag.name, parody_map);
						parodies.push((name, tag.count));
					}
				}
				"character" => {
					let name = translate_name(&tag.name, character_map);
					characters.push((name, tag.count));
				}
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
		let tags: Vec<_> = tags.into_iter().map(|(name, _)| name).collect();
		let groups: Vec<_> = groups.into_iter().map(|(name, _)| name).collect();
		let artists: Vec<_> = artists.into_iter().map(|(name, _)| name).collect();
		let parodies: Vec<_> = parodies.into_iter().map(|(name, _)| name).collect();
		let characters: Vec<_> = characters.into_iter().map(|(name, _)| name).collect();

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
