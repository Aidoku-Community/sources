use aidoku::{
	ContentRating, Manga, MangaStatus, UpdateStrategy, Viewer,
	alloc::{String, Vec},
	prelude::*,
};
use serde::Deserialize;

use crate::{context::Context, endpoints::Url, models::common::LibGroupRating};

use super::common::{
	LibGroupAgeRestriction, LibGroupCover, LibGroupMediaType, LibGroupStatus, LibGroupTag,
};

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupManga {
	pub rus_name: String,
	pub eng_name: Option<String>,
	#[serde(rename = "otherNames")]
	pub other_names: Option<Vec<String>>,
	pub slug_url: String,
	pub cover: LibGroupCover,
	#[serde(rename = "ageRestriction")]
	pub age_restriction: LibGroupAgeRestriction,
	#[serde(rename = "type")]
	pub media_type: LibGroupMediaType,
	pub summary: Option<String>,
	pub rating: Option<LibGroupRating>,
	pub tags: Option<Vec<LibGroupTag>>,
	pub authors: Option<Vec<LibGroupAuthor>>,
	pub artists: Option<Vec<LibGroupAuthor>>,
	pub status: LibGroupStatus,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupAuthor {
	pub name: String,
	pub rus_name: Option<String>,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LibGroupCoverItem {
	pub cover: LibGroupCover,
	pub order: i32,
}

impl LibGroupManga {
	pub fn into_manga(self, ctx: &Context) -> Manga {
		Manga {
			key: self.slug_url.clone(),
			title: if !self.rus_name.is_empty() {
				self.rus_name.clone()
			} else {
				self.eng_name.clone().unwrap_or_default()
			},
			cover: Some(self.cover.get_cover_url(&ctx.cover_quality)),
			artists: self.artists.as_ref().map(|artists| {
				artists
					.iter()
					.map(|author| {
						author
							.rus_name
							.clone()
							.unwrap_or_else(|| author.name.clone())
					})
					.collect()
			}),
			authors: self.authors.as_ref().map(|authors| {
				authors
					.iter()
					.map(|author| {
						author
							.rus_name
							.clone()
							.unwrap_or_else(|| author.name.clone())
					})
					.collect()
			}),
			description: Some(Self::detailed_description(&self)),
			url: Some(Url::manga_page(&ctx.base_url, &self.slug_url)),
			tags: self
				.tags
				.as_ref()
				.map(|tags| tags.iter().map(|tag| tag.name.clone()).collect()),
			status: match self.status.label.as_str() {
				"Онгоинг" => MangaStatus::Ongoing,
				"Завершён" => MangaStatus::Completed,
				"Приостановлен" => MangaStatus::Hiatus,
				"Выпуск прекращён" => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			content_rating: match self.age_restriction.label.as_str() {
				"Нет" | "6+" | "12+" => ContentRating::Safe,
				"16+" => ContentRating::Suggestive,
				"18+" => ContentRating::NSFW,
				_ => ContentRating::Unknown,
			},
			viewer: match self.media_type.label.as_str() {
				"Манга" => Viewer::RightToLeft,
				"Манхва" => Viewer::Webtoon,
				"Маньхуа" => Viewer::Webtoon,
				"Комикс" => Viewer::LeftToRight,
				"Руманга" => Viewer::RightToLeft,
				"OEL-манга" => Viewer::RightToLeft,
				"Япония" | "Корея" | "Китай" | "Английский" | "Авторский" | "Фанфик" => {
					Viewer::Webtoon
				}
				_ => Viewer::Unknown,
			},
			update_strategy: UpdateStrategy::Always,
			..Default::default()
		}
	}

	fn detailed_description(maga: &LibGroupManga) -> String {
		let mut description = String::new();

		// Summary
		if let Some(summary) = &maga.summary
			&& !summary.is_empty()
		{
			description.push_str(summary);
			description.push_str("\n\n");
		}

		// Rating section
		if let Some(rating) = &maga.rating
			&& let Ok(avg) = rating.average.parse::<f32>()
		{
			let stars_count = (avg / 2.0).clamp(0.0, 5.0);

			let full_stars = stars_count as u8;
			let half_star = ((stars_count * 10.0) as u8 % 10) >= 5;
			let empty_stars = 5 - full_stars - if half_star { 1 } else { 0 };

			let full_symbol = "★";
			let half_symbol = "✮";
			let empty_symbol = "☆";

			let mut stars_str = String::new();
			for _ in 0..full_stars {
				stars_str.push_str(full_symbol);
			}
			if half_star {
				stars_str.push_str(half_symbol);
			}
			for _ in 0..empty_stars {
				stars_str.push_str(empty_symbol);
			}

			description.push_str(&format!(
				"{} {:.2} (голосов: {})\n\n",
				stars_str, avg, rating.votes
			));
		}

		// Alternative names
		let mut alt_names: Vec<String> = Vec::new();

		// If both rus and eng names exist, add eng name as first alt
		if !maga.rus_name.is_empty() && !maga.eng_name.clone().unwrap_or_default().is_empty() {
			alt_names.push(maga.eng_name.clone().unwrap_or_default());
		}

		// Append any provided other_names
		if let Some(other_names) = &maga.other_names {
			for name in other_names {
				alt_names.push(name.clone());
			}
		}

		if !alt_names.is_empty() {
			description.push_str("Альтернативные названия:<br/>");
			description.push_str(&alt_names.join(", "));
		}

		description
	}
}
