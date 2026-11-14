use crate::{
	Params,
	helpers::{credit_names, link_keys, series_path, title_case},
	markdown, settings,
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result, Viewer,
	alloc::{String, Vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SeriesResponse {
	pub data: Vec<Series>,
	pub meta: Pagination,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
	pub has_more: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
	pub slug: String,
	pub url_slug: Option<String>,
	pub url: Option<String>,
	pub title: String,
	#[serde(alias = "cover_image")]
	pub cover_image: Option<String>,
	#[serde(default)]
	pub status: String,
	#[serde(default, rename = "type")]
	pub series_type: String,
	#[serde(default)]
	pub is_mature: bool,
	#[serde(default, alias = "is_nsfw")]
	pub nsfw: bool,
	pub description: Option<String>,
	pub author: Option<String>,
	pub artist: Option<String>,
	#[serde(default)]
	pub genres: Vec<Genre>,
	#[serde(default)]
	pub tags: Vec<Genre>,
	#[serde(default)]
	pub chapters: Vec<ChapterDto>,
	#[serde(alias = "chapter_count")]
	pub chapter_count: Option<usize>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Genre {
	Name(String),
	Reference {
		#[serde(default)]
		name: String,
		#[serde(default)]
		slug: String,
		genre: Option<GenreSlug>,
	},
}

#[derive(Deserialize)]
pub struct GenreSlug {
	pub slug: String,
	pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct Facets {
	pub genres: Vec<Facet>,
	pub tags: Vec<Facet>,
}

#[derive(Deserialize)]
pub struct Facet {
	pub name: String,
	pub slug: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
	pub number: f32,
	pub title: Option<String>,
	#[serde(alias = "published_at")]
	pub published_at: Option<String>,
	#[serde(default)]
	pub is_locked: bool,
	pub has_access: Option<bool>,
	#[serde(alias = "is_free")]
	pub is_free: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reader {
	pub chapter: ChapterPages,
	pub is_unlocked: bool,
	pub is_locked: bool,
	pub protection_config: Option<ProtectionConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionConfig {
	pub is_protected: bool,
}

#[derive(Deserialize)]
pub struct ChapterPages {
	pub content: Option<String>,
	#[serde(default)]
	pub pages: Vec<PageDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDto {
	pub image_url: Option<String>,
	#[serde(default)]
	pub is_encrypted: bool,
	#[serde(default)]
	pub has_strips: bool,
	#[serde(default)]
	pub has_fragments: bool,
	#[serde(default)]
	pub is_redacted: bool,
}

impl TryFrom<Series> for Manga {
	type Error = aidoku::AidokuError;

	fn try_from(series: Series) -> Result<Self> {
		let slug = series
			.url
			.as_deref()
			.and_then(link_keys)
			.map(|(key, _)| match key.strip_prefix("novel/") {
				Some(slug) => slug.into(),
				None => key,
			})
			.or(series.url_slug)
			.filter(|s| !s.trim().is_empty())
			.unwrap_or(series.slug);
		let title = series.title.trim();
		if slug.trim().is_empty() || title.is_empty() {
			bail!("Series has no key or title");
		}
		let novel =
			matches!(
				series.series_type.as_str(),
				"NOVEL"
					| "LIGHT_NOVEL" | "WEB_NOVEL"
					| "PUBLISHED_NOVEL"
					| "FANFICTION" | "ORIGINAL_FICTION"
					| "ONE_SHOT"
			);
		let key = if novel { format!("novel/{slug}") } else { slug };
		let mut tags: Vec<String> = series
			.genres
			.into_iter()
			.chain(series.tags)
			.filter_map(|genre| {
				let name = match genre {
					Genre::Name(name) => name,
					Genre::Reference { name, slug, genre } => {
						if name.is_empty() {
							genre
								.map(|g| g.name.unwrap_or(g.slug))
								.unwrap_or(slug)
								.replace('-', " ")
						} else {
							name
						}
					}
				};
				(!name.trim().is_empty()).then_some(name)
			})
			.map(|name| title_case(name.trim()))
			.collect();
		tags.sort_unstable_by_key(|tag| tag.to_lowercase());
		tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
		let adult = series.nsfw
			|| series.is_mature
			|| tags
				.iter()
				.any(|t| matches!(t.to_lowercase().as_str(), "adult" | "hentai" | "smut"));
		let description = series
			.description
			.filter(|s| !s.trim().is_empty())
			.map(|s| markdown::parse(&s.replace("\\r\\n", "\n").replace("\\n", "\n")))
			.transpose()?
			.filter(|s| !s.is_empty());
		Ok(Manga {
			url: Some(format!("/series/{}", series_path(&key))),
			key,
			title: if novel && settings::label_novels() {
				format!("{title} (Novel)")
			} else {
				title.into()
			},
			cover: series.cover_image.filter(|s| !s.trim().is_empty()),
			authors: credit_names(series.author),
			artists: credit_names(series.artist),
			description,
			tags: Some(tags),
			status: match series.status.as_str() {
				"ONGOING" | "UPCOMING" => MangaStatus::Ongoing,
				"COMPLETED" => MangaStatus::Completed,
				"HIATUS" => MangaStatus::Hiatus,
				"CANCELLED" | "CANCELED" | "DROPPED" | "DISCONTINUED" => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			content_rating: if adult {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			},
			viewer: if novel {
				Viewer::Unknown
			} else {
				match series.series_type.as_str() {
					"MANGA" => Viewer::RightToLeft,
					"MANHWA" | "MANHUA" | "WEBTOON" => Viewer::Webtoon,
					_ => Viewer::Unknown,
				}
			},
			..Default::default()
		})
	}
}

impl ChapterDto {
	pub fn locked(&self) -> bool {
		self.has_access == Some(false)
			|| (self.is_locked || self.is_free == Some(false)) && self.has_access != Some(true)
	}

	pub fn into_chapter(self, params: &Params, manga_key: &str) -> Chapter {
		let key = format!("{}", self.number);
		let locked = self.locked();
		Chapter {
			url: Some(format!(
				"{}/series/{}/chapter/{key}",
				params.base_url,
				series_path(manga_key)
			)),
			title: self.title.filter(|title| {
				!title.trim().is_empty() && title != &key && title != &format!("Chapter {key}")
			}),
			key,
			chapter_number: Some(self.number),
			locked,
			date_uploaded: self.published_at.as_deref().and_then(|date| {
				let date = date.strip_prefix("$D").unwrap_or(date);
				let date = date
					.split_once('.')
					.map_or(date.trim_end_matches('Z'), |(date, _)| date);
				parse_date(date.replace('T', " "), "yyyy-MM-dd HH:mm:ss")
			}),
			..Default::default()
		}
	}
}

pub fn compact(manga: Manga) -> Manga {
	Manga {
		key: manga.key,
		title: manga.title,
		cover: manga.cover,
		status: manga.status,
		content_rating: manga.content_rating,
		..Default::default()
	}
}
