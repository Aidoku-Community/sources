use crate::BASE_URL;
use aidoku::{
	AidokuError, Chapter, ContentRating, Manga, MangaStatus, Result, Viewer,
	alloc::{
		format,
		string::{String, ToString},
		vec::Vec,
	},
	prelude::*,
};
use prost::Message;

#[derive(Message)]
pub struct MangaPlusResponse {
	#[prost(message, optional, tag = "1")]
	pub success: Option<SuccessResult>,
	#[prost(message, optional, tag = "2")]
	pub error: Option<ErrorResult>,
}

impl MangaPlusResponse {
	pub fn decode(data: &[u8]) -> Result<Self> {
		<Self as Message>::decode(data)
			.map_err(|error| error!("Invalid protobuf response: {error}"))
	}

	pub fn result_or_error<T: AsRef<str>>(self, fallback: T) -> Result<SuccessResult> {
		self.success.ok_or_else(|| {
			AidokuError::Message(
				self.error
					.and_then(ErrorResult::body)
					.unwrap_or_else(|| fallback.as_ref().into()),
			)
		})
	}
}

#[derive(Message)]
pub struct ErrorResult {
	#[prost(message, optional, tag = "2")]
	english_popup: Option<Popup>,
	#[prost(message, optional, tag = "3")]
	spanish_popup: Option<Popup>,
}
impl ErrorResult {
	fn body(self) -> Option<String> {
		self.english_popup
			.or(self.spanish_popup)
			.map(|popup| popup.body)
	}
}

#[derive(Message)]
struct Popup {
	#[prost(string, tag = "2")]
	body: String,
}

#[derive(Message)]
pub struct SuccessResult {
	#[prost(message, optional, tag = "8")]
	pub title_detail_view: Option<TitleDetailView>,
	#[prost(message, optional, tag = "10")]
	pub manga_viewer: Option<MangaViewer>,
	#[prost(message, optional, tag = "35")]
	pub all_titles_view_v3: Option<AllTitlesViewV3>,
	#[prost(message, optional, tag = "37")]
	pub title_ranking_view: Option<TitleRankingView>,
	#[prost(message, optional, tag = "38")]
	pub web_home_view: Option<WebHomeView>,
}

#[derive(Message)]
pub struct AllTitlesViewV3 {
	#[prost(message, repeated, tag = "3")]
	pub titles: Vec<AllTitlesV3Entry>,
}

#[derive(Message)]
pub struct AllTitlesV3Entry {
	#[prost(message, required, tag = "2")]
	pub title: Title,
}

#[derive(Message)]
pub struct TitleRankingView {
	#[prost(message, repeated, tag = "3")]
	pub ranked_titles: Vec<RankedTitle>,
}

#[derive(Message)]
pub struct RankedTitle {
	#[prost(message, repeated, tag = "2")]
	pub titles: Vec<Title>,
}

#[derive(Message)]
pub struct WebHomeView {
	#[prost(message, repeated, tag = "2")]
	pub groups: Vec<UpdatedTitleGroup>,
}

#[derive(Message)]
pub struct UpdatedTitleGroup {
	#[prost(message, repeated, tag = "2")]
	pub titles: Vec<UpdatedTitle>,
}

#[derive(Message)]
pub struct UpdatedTitle {
	#[prost(message, optional, tag = "3")]
	latest_chapter: Option<LatestChapter>,
}
impl UpdatedTitle {
	pub fn title(self) -> Option<Title> {
		self.latest_chapter.and_then(|chapter| chapter.title)
	}
}

#[derive(Message)]
struct LatestChapter {
	#[prost(message, optional, tag = "1")]
	title: Option<Title>,
}

#[derive(Clone, Message)]
pub struct Title {
	#[prost(int32, tag = "1")]
	pub title_id: i32,
	#[prost(string, tag = "2")]
	pub name: String,
	#[prost(string, optional, tag = "3")]
	pub author: Option<String>,
	#[prost(string, tag = "4")]
	pub portrait_image_url: String,
	#[prost(enumeration = "Language", optional, tag = "7")]
	language_code: Option<i32>,
}
impl Title {
	pub fn language(&self) -> Option<Language> {
		self.language_code
			.and_then(|value| Language::try_from(value).ok())
	}
}
impl From<Title> for Manga {
	fn from(value: Title) -> Self {
		Self {
			key: value.title_id.to_string(),
			title: value.name,
			cover: Some(value.portrait_image_url),
			authors: value.author.map(|author| {
				author
					.split(['/', ','])
					.map(|part| part.trim().into())
					.collect()
			}),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum Language {
	English = 0,
	Spanish = 1,
	French = 2,
	Indonesian = 3,
	BrazilianPortuguese = 4,
	Russian = 5,
	Thai = 6,
	German = 7,
	Vietnamese = 9,
}

#[derive(Message)]
pub struct TitleDetailView {
	#[prost(message, required, tag = "1")]
	pub title: Title,
	#[prost(string, optional, tag = "3")]
	pub overview: Option<String>,
	#[prost(string, tag = "7")]
	pub viewing_period_description: String,
	#[prost(string, tag = "8")]
	pub non_appearance_info: String,
	#[prost(message, repeated, tag = "28")]
	pub chapter_list_group: Vec<ChapterListGroup>,
}
impl TitleDetailView {
	pub fn chapter_list(&self) -> Vec<&MangaPlusChapter> {
		self.chapter_list_group
			.iter()
			.flat_map(|group| {
				group
					.first_chapter_list
					.iter()
					.chain(group.last_chapter_list.iter())
			})
			.collect()
	}
	fn is_oneshot(&self) -> bool {
		self.chapter_list().len() == 1 && self.chapter_list()[0].name.to_lowercase() == "one-shot"
	}
	fn is_completed(&self) -> bool {
		self.non_appearance_info.to_lowercase().contains("complet")
			|| self.is_oneshot()
			|| self
				.viewing_period_description
				.contains("latest 0 chapters")
	}
	fn is_on_hiatus(&self) -> bool {
		self.non_appearance_info
			.to_lowercase()
			.contains("on a hiatus")
	}
}
impl From<TitleDetailView> for Manga {
	fn from(value: TitleDetailView) -> Self {
		let description = format!(
			"{}\n\n{}",
			value.overview.as_deref().unwrap_or_default(),
			if value.is_completed() {
				""
			} else {
				&value.viewing_period_description
			},
		)
		.trim()
		.into();
		let status = if value.is_completed() {
			MangaStatus::Completed
		} else if value.is_on_hiatus() {
			MangaStatus::Hiatus
		} else {
			MangaStatus::Ongoing
		};
		let base: Manga = value.title.into();
		Manga {
			description: Some(description),
			url: Some(format!("{BASE_URL}/titles/{}", base.key)),
			status,
			content_rating: ContentRating::Safe,
			viewer: Viewer::RightToLeft,
			..base
		}
	}
}

#[derive(Message)]
pub struct ChapterListGroup {
	#[prost(message, repeated, tag = "2")]
	pub first_chapter_list: Vec<MangaPlusChapter>,
	#[prost(message, repeated, tag = "4")]
	pub last_chapter_list: Vec<MangaPlusChapter>,
}

#[derive(Clone, Message)]
pub struct MangaPlusChapter {
	#[prost(int32, tag = "2")]
	pub chapter_id: i32,
	#[prost(string, tag = "3")]
	pub name: String,
	#[prost(string, optional, tag = "4")]
	pub sub_title: Option<String>,
	#[prost(int64, tag = "6")]
	pub start_time_stamp: i64,
}
impl MangaPlusChapter {
	pub fn is_expired(&self) -> bool {
		self.sub_title.is_none()
	}
}
impl From<MangaPlusChapter> for Chapter {
	fn from(value: MangaPlusChapter) -> Self {
		let chapter_number = value
			.name
			.find('#')
			.and_then(|idx| value.name[idx + 1..].parse::<f32>().ok());
		Chapter {
			key: value.chapter_id.to_string(),
			title: Some(value.sub_title.unwrap_or(value.name)),
			chapter_number,
			date_uploaded: Some(value.start_time_stamp),
			url: Some(format!("{BASE_URL}/viewer/{}", value.chapter_id)),
			..Default::default()
		}
	}
}

#[derive(Message)]
pub struct MangaViewer {
	#[prost(message, repeated, tag = "1")]
	pub pages: Vec<MangaPlusPage>,
	#[prost(int32, optional, tag = "9")]
	pub title_id: Option<i32>,
	#[prost(string, optional, tag = "19")]
	pub view_token: Option<String>,
}

#[derive(Message)]
pub struct MangaPlusPage {
	#[prost(message, optional, tag = "1")]
	pub manga_page: Option<MangaPage>,
}

#[derive(Message)]
pub struct MangaPage {
	#[prost(string, tag = "1")]
	pub image_url: String,
	#[prost(string, optional, tag = "5")]
	pub encryption_key: Option<String>,
}
