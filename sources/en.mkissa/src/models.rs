use crate::helpers::{
	content_rating, cover_url, description_text, display_title, manga_url, parse_status, tag_names,
	viewer,
};
use aidoku::{
	AidokuError, ContentRating, Manga, Result,
	alloc::{String, Vec},
	prelude::*,
};
use serde::Deserialize;

pub const BASE_URL: &str = "https://mkissa.to";
pub const API_URL: &str = "https://api.mkissa.net/api";
pub const CDN_URL: &str = "https://wp.youtube-anime.com";

pub const PAGE_SIZE: i32 = 20;

#[derive(Deserialize)]
pub struct GraphQLResponse<T> {
	pub data: Option<T>,
	pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Deserialize)]
pub struct GraphQLError {
	pub message: String,
}

#[derive(Deserialize)]
pub struct DateParts {
	pub year: Option<i32>,
	pub month: Option<i32>,
	pub date: Option<i32>,
	pub hour: Option<i32>,
	pub minute: Option<i32>,
	pub second: Option<i32>,
}

#[derive(Deserialize)]
pub struct Translations<T> {
	pub sub: Option<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastChapter {
	pub chapter_string: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaCard {
	#[serde(rename = "_id")]
	pub id: String,
	#[serde(default)]
	pub name: String,
	pub english_name: Option<String>,
	pub thumbnail: Option<String>,
	pub is_adult: Option<bool>,
	pub description: Option<String>,
	pub authors: Option<Vec<String>>,
	pub genres: Option<Vec<String>>,
	pub tags: Option<Vec<String>>,
	pub status: Option<String>,
	#[serde(rename = "type")]
	pub kind: Option<String>,
	pub country_of_origin: Option<String>,
	pub last_chapter_info: Option<Translations<LastChapter>>,
	pub last_chapter_date: Option<Translations<DateParts>>,
}

impl MangaCard {
	pub fn into_details(mut self) -> Result<Manga> {
		let tags = tag_names(
			self.genres
				.take()
				.unwrap_or_default()
				.into_iter()
				.chain(self.tags.take().unwrap_or_default())
				.collect(),
		);
		let mut authors: Vec<String> = Vec::new();
		for author in self.authors.take().unwrap_or_default() {
			let name = author.trim();
			if !name.is_empty() && !authors.iter().any(|existing| existing == name) {
				authors.push(name.into());
			}
		}
		Ok(Manga {
			description: self
				.description
				.take()
				.and_then(|description| description_text(&description, true)),
			authors: (!authors.is_empty()).then_some(authors),
			status: parse_status(self.status.as_deref()),
			content_rating: content_rating(&tags, self.is_adult),
			viewer: viewer(
				self.kind.as_deref(),
				self.country_of_origin.as_deref(),
				&tags,
			),
			url: Some(manga_url(&self.id)),
			tags: (!tags.is_empty()).then_some(tags),
			..Manga::try_from(self)?
		})
	}

	pub fn into_featured(mut self) -> Result<Manga> {
		let tags = tag_names(self.genres.take().unwrap_or_default());
		let description = self
			.description
			.take()
			.and_then(|description| description_text(&description, false));
		let status = parse_status(self.status.as_deref());
		let content_rating = content_rating(&tags, self.is_adult);
		Ok(Manga {
			description,
			status,
			content_rating,
			tags: (!tags.is_empty()).then_some(tags),
			..Manga::try_from(self)?
		})
	}
}

impl TryFrom<MangaCard> for Manga {
	type Error = AidokuError;

	fn try_from(card: MangaCard) -> Result<Self> {
		let title = display_title(card.english_name.as_deref(), &card.name)?;
		if card.id.trim().is_empty() {
			bail!("Series has no key");
		}
		Ok(Manga {
			title,
			cover: cover_url(card.thumbnail.as_deref()),
			content_rating: if card.is_adult == Some(true) {
				ContentRating::NSFW
			} else {
				ContentRating::Unknown
			},
			key: card.id,
			..Default::default()
		})
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
	pub any_card: Option<MangaCard>,
}

#[derive(Deserialize)]
pub struct Recommendations {
	pub total: Option<i32>,
	pub recommendations: Vec<Recommendation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopularData {
	pub query_popular: Recommendations,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendingData {
	pub query_latest_page_status: Recommendations,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomData {
	pub query_random_recommendation: Option<Vec<MangaCard>>,
}

#[derive(Deserialize)]
pub struct PageInfo {
	pub total: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaEdges {
	pub page_info: PageInfo,
	pub edges: Vec<MangaCard>,
}

#[derive(Deserialize)]
pub struct SearchData {
	pub mangas: MangaEdges,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptationsData {
	pub season_adaptation_mangas: MangaEdges,
}

#[derive(Deserialize)]
pub struct Tag {
	pub name: String,
	pub slug: String,
}

#[derive(Deserialize)]
pub struct TagEdges {
	pub edges: Vec<Tag>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagsData {
	pub query_tags: TagEdges,
}

#[derive(Deserialize)]
pub struct DetailsData {
	pub manga: Option<MangaCard>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChaptersManga {
	pub available_chapters_detail: Option<Translations<Vec<String>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeInfo {
	pub episode_id_num: serde_json::Value,
	pub notes: Option<String>,
	pub upload_dates: Option<Translations<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChaptersData {
	pub manga: Option<ChaptersManga>,
	pub episode_infos: Option<Vec<EpisodeInfo>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum PictureUrl {
	Str(String),
	Obj { url: Option<String> },
}

impl PictureUrl {
	pub fn url(&self) -> Option<&str> {
		match self {
			PictureUrl::Str(s) => Some(s),
			PictureUrl::Obj { url } => url.as_deref(),
		}
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPageEdge {
	pub picture_url_head: Option<String>,
	pub picture_urls: Option<Vec<PictureUrl>>,
}

#[derive(Deserialize)]
pub struct ChapterPages {
	pub edges: Vec<ChapterPageEdge>,
}

// The reader's payload, captured with or without its GraphQL `data` wrapper
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderPayload {
	pub chapter_pages: Option<ChapterPages>,
	pub data: Option<ReaderData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderData {
	pub chapter_pages: Option<ChapterPages>,
}
