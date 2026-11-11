use aidoku::{
	ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, borrow::Cow, format, vec},
};
use serde::Deserialize;

#[derive(Debug)]
pub struct Hosts {
	pub toon_list: String,
	pub webtoon: String,
	pub image: String,
}

#[derive(Debug, Deserialize)]
pub struct ChapterItem {
	pub id: String,
	#[serde(rename = "t")]
	pub title: String,
	#[serde(rename = "d", default)]
	pub date: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SeriesItem {
	#[serde(rename = "x")]
	pub id: String,
	#[serde(rename = "t")]
	pub title: String,
	#[serde(rename = "p", default)]
	pub poster: String,
	#[serde(rename = "au", default)]
	pub author: String,
	#[serde(rename = "g", default)]
	pub updated_at: i64,
	#[serde(rename = "h", default)]
	pub hot: i64,
	#[serde(rename = "pd", default)]
	pub pday: String,
	#[serde(rename = "tag", default)]
	pub tag: String,
	#[serde(rename = "ag", default)]
	pub age_rating: String,
	#[serde(skip)]
	pub is_completed: bool,
}

impl SeriesItem {
	pub fn to_manga(&self, image_cdn: &str, base_url: &str) -> Manga {
		let cover = if !self.poster.is_empty() {
			let clean: Cow<str> = if self.poster.contains("_x4") {
				Cow::Owned(self.poster.replace("_x4", ""))
			} else if self.poster.contains("_x3") {
				Cow::Owned(self.poster.replace("_x3", ""))
			} else {
				Cow::Borrowed(&self.poster)
			};
			Some(format!(
				"{}/{}",
				image_cdn.trim_end_matches('/'),
				clean.trim_start_matches('/')
			))
		} else {
			None
		};

		let content_rating = if self.age_rating == "19" {
			ContentRating::NSFW
		} else {
			ContentRating::Safe
		};

		Manga {
			key: self.id.clone(),
			title: self.title.clone(),
			cover,
			authors: (!self.author.is_empty()).then(|| vec![self.author.clone()]),
			url: Some(format!(
				"{}/webtoon/{}.html",
				base_url.trim_end_matches('/'),
				self.id
			)),
			status: if self.is_completed {
				MangaStatus::Completed
			} else {
				MangaStatus::Ongoing
			},
			content_rating,
			viewer: Viewer::Webtoon,
			..Default::default()
		}
	}
}
