use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, format, vec},
	imports::std::parse_date,
};
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct SeriesResponse {
	pub data: Vec<Series>,
	pub meta: Pagination,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Pagination {
	pub has_more: bool,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Series {
	pub id: String,
	pub slug: String,
	pub title: String,
	pub cover_image: Option<String>,
	pub status: String,
	#[serde(rename = "type")]
	pub series_type: String,
	pub origin: String,
	pub rating: f64,
	pub is_hot: bool,
	pub is_mature: bool,
	pub original_title: Option<String>,
	pub aliases: Vec<String>,
	pub description: Option<String>,
	pub genres: Vec<Genre>,
	pub team: Option<Team>,
	pub similar_series: Vec<Series>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Genre {
	pub name: String,
	pub slug: String,
	pub genre: Option<GenreSlug>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct GenreSlug {
	pub slug: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Team {
	pub name: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Detail {
	pub series: Series,
	pub chapters: Vec<ChapterDto>,
	pub total_pages: i32,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChapterDto {
	pub id: String,
	pub number: f32,
	pub title: Option<String>,
	pub published_at: Option<String>,
	pub is_locked: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ChapterDetail {
	pub chapter: ChapterPages,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ChapterPages {
	pub pages: Vec<PageDto>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PageDto {
	pub image_url: Option<String>,
}

impl Series {
	pub fn into_basic_manga(self, base_url: &str) -> Manga {
		Manga {
			key: self.slug,
			title: self.title,
			cover: self.cover_image.map(|url| absolute_url(base_url, &url)),
			..Default::default()
		}
	}

	pub fn into_manga(self, base_url: &str) -> Manga {
		let tags = {
			let mut tags = Vec::new();
			if !self.origin.is_empty() {
				tags.push(self.origin);
			}
			for genre in self.genres {
				let value = if genre.name.is_empty() {
					genre.genre.map(|it| it.slug).unwrap_or(genre.slug)
				} else {
					genre.name
				};
				if !value.is_empty() && !tags.contains(&value) {
					tags.push(value);
				}
			}
			tags
		};

		let description = {
			let mut description = self.description.unwrap_or_default();
			let mut info = Vec::new();
			if self.rating > 0.0 {
				info.push(format!("Rating: {}", self.rating));
			}
			if self.is_hot {
				info.push("Featured".into());
			}
			let mut aliases = Vec::new();
			if let Some(title) = self
				.original_title
				.filter(|title| !title.is_empty() && title != &self.title)
			{
				aliases.push(title);
			}
			for title in self.aliases {
				if !title.is_empty() && title != self.title && !aliases.contains(&title) {
					aliases.push(title);
				}
			}
			if !info.is_empty() {
				if !description.is_empty() {
					description.push_str("\n\n");
				}
				description.push_str(&info.join("\n"));
			}
			if !aliases.is_empty() {
				if !description.is_empty() {
					description.push_str("\n\n");
				}
				description.push_str("Alternative titles:\n- ");
				description.push_str(&aliases.join("\n- "));
			}
			description
		};

		let url = format!("{base_url}/series/comic/{}", self.slug);

		Manga {
			key: self.slug,
			title: self.title,
			cover: self.cover_image.map(|url| absolute_url(base_url, &url)),
			authors: self.team.and_then(|team| team.name).map(|name| vec![name]),
			description: (!description.is_empty()).then_some(description),
			tags: (!tags.is_empty()).then_some(tags),
			status: parse_status(&self.status),
			content_rating: if self.is_mature {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			},
			viewer: match self.series_type.as_str() {
				"MANGA" => Viewer::RightToLeft,
				"MANHWA" | "MANHUA" => Viewer::Webtoon,
				_ => Viewer::Unknown,
			},
			url: Some(url),
			..Default::default()
		}
	}
}

impl ChapterDto {
	pub fn into_chapter(self, base_url: &str, slug: &str) -> Chapter {
		let number = format_number(self.number);
		let title = self
			.title
			.filter(|title| !title.is_empty() && title != &number);
		let date_uploaded = self
			.published_at
			.as_deref()
			.and_then(|d| parse_date(d, "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'"));
		let url = format!("{base_url}/series/comic/{slug}/chapter/{number}");
		Chapter {
			key: number,
			title,
			chapter_number: Some(self.number),
			date_uploaded,
			url: Some(url),
			..Default::default()
		}
	}
}

pub fn absolute_url(base_url: &str, url: &str) -> String {
	if url.starts_with("http://") || url.starts_with("https://") {
		url.into()
	} else {
		format!("{base_url}{url}")
	}
}

fn format_number(number: f32) -> String {
	if number == number as i32 as f32 {
		format!("{}", number as i32)
	} else {
		format!("{number}")
	}
}

fn parse_status(status: &str) -> MangaStatus {
	match status {
		"ONGOING" => MangaStatus::Ongoing,
		"COMPLETED" => MangaStatus::Completed,
		"HIATUS" => MangaStatus::Hiatus,
		"CANCELLED" | "CANCELED" | "DROPPED" | "DISCONTINUED" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}
