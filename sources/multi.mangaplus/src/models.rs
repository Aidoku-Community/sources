use crate::BASE_URL;
use crate::settings;
use aidoku::{
	AidokuError, Chapter, ContentRating, Manga, MangaStatus, Result, Viewer,
	alloc::{
		format,
		string::{String, ToString},
		vec::Vec,
	},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct MangaPlusResponse {
	pub success: Option<SuccessResult>,
	pub error: Option<ErrorResult>,
}

impl MangaPlusResponse {
	pub fn result_or_error<T: AsRef<str>>(self, fallback: T) -> Result<SuccessResult> {
		self.success.ok_or_else(|| {
			let error = self
				.error
				.and_then(|result| result.get(Language::English))
				.map(|popup| popup.body)
				.unwrap_or(fallback.as_ref().into());
			AidokuError::Message(error)
		})
	}
}

#[derive(Deserialize)]
pub struct ErrorResult {
	#[serde(default)]
	pub popups: Vec<Popup>,
}

impl ErrorResult {
	pub fn get(self, language: Language) -> Option<Popup> {
		self.popups
			.into_iter()
			.find(|popup| popup.language == Some(language))
	}
}

#[derive(Deserialize)]
pub struct Popup {
	// pub subject: String,
	pub body: String,
	#[serde(default = "default_language")]
	pub language: Option<Language>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessResult {
	// pub is_featured_updated: Option<bool>,
	// pub title_ranking_view_v2: Option<TitleRankingViewV2>,
	pub title_detail_view: Option<TitleDetailView>,
	pub manga_viewer: Option<MangaViewer>,
	pub all_titles_view_v2: Option<AllTitlesViewV2>,
	pub web_home_view_v4: Option<WebHomeViewV4>,
}

// #[derive(Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct TitleRankingViewV2 {
// 	pub ranked_titles: Vec<RankedTitle>,
// }

#[derive(Deserialize)]
pub struct RankedTitle {
	pub titles: Vec<Title>,
}

#[derive(Deserialize)]
pub struct AllTitlesViewV2 {
	#[serde(rename = "AllTitlesGroup")]
	pub all_titles_group: Vec<AllTitlesGroup>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllTitlesGroup {
	// pub the_title: String,
	#[serde(default)]
	pub titles: Vec<Title>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct WebHomeViewV4 {
	pub top_banners: Vec<Banner>,
	pub groups: Vec<UpdatedTitleV2Group>,
	pub ranked_titles: Vec<RankedTitle>,
	// pub featured_title_lists: Vec<FeaturedTitleList>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Banner {
	pub image_url: String,
	pub action: BannerAction,
	// pub id: i32,
}

#[derive(Deserialize)]
pub struct BannerAction {
	// pub method: Option<String>,
	pub url: String,
}

// #[derive(Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct FeaturedTitleList {
// 	pub featured_titles: Vec<Title>,
// }

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TitleDetailView {
	pub title: Title,
	pub title_image_url: String,
	pub overview: Option<String>,
	#[serde(default)]
	pub next_time_stamp: i64,
	#[serde(default)]
	pub viewing_period_description: String,
	#[serde(default)]
	pub non_appearance_info: String,
	#[serde(default)]
	pub chapter_list_group: Vec<ChapterListGroup>,
	#[serde(default)]
	pub chapter_list_v2: Vec<MangaPlusChapter>,
	#[serde(default)]
	pub is_simul_released: bool,
	#[serde(default)]
	pub rating: Rating,
	#[serde(default = "default_true")]
	pub chapters_descending: bool,
	#[serde(default)]
	pub title_labels: TitleLabels,
	#[serde(default = "default_wsj")]
	pub label: Option<Label>,
}

impl TitleDetailView {
	pub fn chapter_list(&self) -> Vec<&MangaPlusChapter> {
		if settings::get_mobile() {
			self.chapter_list_v2.iter().collect()
		} else {
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
	}

	fn is_webtoon(&self) -> bool {
		false
	}

	fn is_oneshot(&self) -> bool {
		let chapter_list = self.chapter_list();
		chapter_list.len() == 1 && chapter_list[0].name.to_lowercase() == "one-shot"
	}

	fn is_reedition(&self) -> bool {
		self.viewing_period_description.contains("revival")
			|| self.viewing_period_description.contains("remasterizada")
	}

	fn is_completed(&self) -> bool {
		self.non_appearance_info.contains("complet") // completado|complete|completo
			|| self.is_oneshot()
			|| self.title_labels.release_schedule == ReleaseSchedule::Completed
			|| self.title_labels.release_schedule == ReleaseSchedule::Disabled
	}

	fn is_simulpub(&self) -> bool {
		self.is_simul_released || self.title_labels.is_simulpub
	}

	fn is_on_hiatus(&self) -> bool {
		self.non_appearance_info
			.to_lowercase()
			.contains("on a hiatus")
	}

	fn genres(&self) -> Vec<String> {
		let mut genres = Vec::new();

		let is_oneshot = self.is_oneshot();
		let is_reedition = self.is_reedition();
		let is_completed = self.is_completed();

		if self.is_simulpub() && !is_reedition && !is_completed {
			genres.push("Simulrelease".into());
		}

		if is_oneshot {
			genres.push("One-Shot".into());
		}

		if is_reedition {
			genres.push("Re-edition".into());
		}

		if self.is_webtoon() {
			genres.push("Webtoon".into());
		}

		if let Some(magazine) = self.label.as_ref().and_then(|l| l.magazine()) {
			genres.push(magazine.into());
		}

		if !is_completed {
			genres.push(self.title_labels.release_schedule.to_str().into());
		}

		genres.push(self.rating.to_str().into());

		genres
	}

	fn viewing_information(&self) -> Option<&String> {
		if !self.is_completed() {
			Some(&self.viewing_period_description)
		} else {
			None
		}
	}
}

impl From<TitleDetailView> for Manga {
	fn from(value: TitleDetailView) -> Self {
		let description = format!(
			"{}\n\n{}",
			value.overview.as_ref().unwrap_or(&String::default()),
			value.viewing_information().unwrap_or(&String::default())
		)
		.trim()
		.into();
		let genres = value.genres();
		let status = if value.is_completed() {
			MangaStatus::Completed
		} else if value.is_on_hiatus() {
			MangaStatus::Hiatus
		} else {
			MangaStatus::Ongoing
		};
		let viewer = if value.is_webtoon() {
			Viewer::Webtoon
		} else {
			Viewer::RightToLeft
		};
		let base: Manga = value.title.into();
		Manga {
			description: Some(description),
			url: Some(format!("{BASE_URL}/titles/{}", base.key)),
			tags: Some(genres),
			status,
			content_rating: if value.rating == Rating::Mature {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			},
			viewer,
			next_update_time: if value.next_time_stamp != 0 {
				Some(value.next_time_stamp)
			} else {
				None
			},
			..base
		}
	}
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct TitleLabels {
	pub release_schedule: ReleaseSchedule,
	pub is_simulpub: bool,
}

#[derive(Deserialize, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReleaseSchedule {
	#[default]
	Disabled,
	Everyday,
	Weekly,
	Biweekly,
	Monthly,
	Bimonthly,
	Trimonthly,
	Other,
	Completed,
}

impl ReleaseSchedule {
	fn to_str(&self) -> &'static str {
		match self {
			ReleaseSchedule::Disabled => "Disabled",
			ReleaseSchedule::Everyday => "Everyday",
			ReleaseSchedule::Weekly => "Weekly",
			ReleaseSchedule::Biweekly => "Biweekly",
			ReleaseSchedule::Monthly => "Monthly",
			ReleaseSchedule::Bimonthly => "Bimonthly",
			ReleaseSchedule::Trimonthly => "Trimonthly",
			ReleaseSchedule::Other => "Other",
			ReleaseSchedule::Completed => "Completed",
		}
	}
}

#[derive(Deserialize, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum Rating {
	#[default]
	AllAge,
	Teen,
	TeenPlus,
	Mature,
}

impl Rating {
	fn to_str(&self) -> &'static str {
		match self {
			Rating::AllAge => "All Ages",
			Rating::Teen => "Teen",
			Rating::TeenPlus => "Teen+",
			Rating::Mature => "Mature",
		}
	}
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Label {
	#[serde(default = "default_wsj_code")]
	pub label: Option<LabelCode>,
}

impl Label {
	fn magazine(&self) -> Option<&'static str> {
		match self.label {
			Some(LabelCode::WeeklyShonenJump) => Some("Weekly Shounen Jump"),
			Some(LabelCode::JumpSquare) => Some("Jump SQ."),
			Some(LabelCode::VJump) => Some("V Jump"),
			Some(LabelCode::Giga) => Some("Shounen Jump GIGA"),
			Some(LabelCode::WeeklyYoungJump) => Some("Weekly Young Jump"),
			Some(LabelCode::TonariNoYoungJump) => Some("Tonari no Young Jump"),
			Some(LabelCode::JPlus) => Some("Shounen Jump+"),
			Some(LabelCode::Creators) => Some("MANGA Plus Creators"),
			Some(LabelCode::SaikyoJump) => Some("Saikyou Jump"),
			Some(LabelCode::UltraJump) => Some("Ultra Jump"),
			Some(LabelCode::DashX) => Some("Dash X Comic"),
			Some(LabelCode::MangaMee) => Some("Manga Mee"),
			_ => None,
		}
	}
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum LabelCode {
	Creators,
	Giga,
	#[serde(rename = "J_PLUS")]
	JPlus,
	Others,
	Revival,
	#[serde(rename = "SKJ")]
	SaikyoJump,
	#[serde(rename = "SQ")]
	JumpSquare,
	#[serde(rename = "TYJ")]
	TonariNoYoungJump,
	#[serde(rename = "VJ")]
	VJump,
	#[serde(rename = "YJ")]
	WeeklyYoungJump,
	#[serde(rename = "WSJ")]
	WeeklyShonenJump,
	#[serde(rename = "UJ")]
	UltraJump,
	#[serde(rename = "DX")]
	DashX,
	#[serde(rename = "MEE")]
	MangaMee,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ChapterListGroup {
	pub first_chapter_list: Vec<MangaPlusChapter>,
	// pub mid_chapter_list: Vec<MangaPlusChapter>,
	pub last_chapter_list: Vec<MangaPlusChapter>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct MangaViewer {
	pub pages: Vec<MangaPlusPage>,
	pub title_id: Option<i32>,
	pub title_name: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Title {
	pub title_id: i32,
	pub name: String,
	pub author: Option<String>,
	pub portrait_image_url: String,
	#[serde(default)]
	pub view_count: i32,
	#[serde(default = "default_language")]
	pub language: Option<Language>,
}

impl From<Title> for Manga {
	fn from(value: Title) -> Self {
		Self {
			key: value.title_id.to_string(),
			title: value.name,
			cover: Some(value.portrait_image_url),
			authors: value.author.map(|s| {
				s.split(['/', ','])
					.map(|part| part.trim())
					.map(String::from)
					.collect()
			}),
			..Default::default()
		}
	}
}

#[derive(Deserialize, Default, PartialEq, Clone, Copy, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum Language {
	#[default]
	English,
	Spanish,
	French,
	Indonesian,
	#[serde(rename = "PORTUGUESE_BR")]
	BrazilianPortuguese,
	Russian,
	Thai,
	Vietnamese,
	German,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedTitleV2Group {
	pub group_name: String,
	#[serde(default)]
	pub title_groups: Vec<OriginalTitleGroup>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginalTitleGroup {
	// pub the_title: String,
	#[serde(default)]
	pub titles: Vec<UpdatedTitle>,
}

#[derive(Deserialize, Clone)]
pub struct UpdatedTitle {
	pub title: Title,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MangaPlusChapter {
	pub title_id: i32,
	pub chapter_id: i32,
	pub name: String,
	pub sub_title: Option<String>,
	pub start_time_stamp: i64,
	pub end_time_stamp: i64,
	#[serde(default)]
	pub is_vertical_only: bool,
}

impl MangaPlusChapter {
	pub fn is_expired(&self) -> bool {
		self.sub_title.is_none()
	}
}

impl From<MangaPlusChapter> for Chapter {
	fn from(value: MangaPlusChapter) -> Self {
		let chapter_number = if let Some(idx) = value.name.find('#') {
			value.name[idx + 1..].parse::<f32>().ok()
		} else {
			None
		};
		Chapter {
			key: value.chapter_id.to_string(),
			title: Some(if let Some(sub_title) = value.sub_title {
				if let Some(stripped_title) =
					chapter_number.and_then(|num| sub_title.strip_prefix(&format!("Chapter {num}")))
				{
					if let Some(final_title) = stripped_title.strip_prefix(": ") {
						final_title.into()
					} else {
						stripped_title.into()
					}
				} else {
					sub_title
				}
			} else {
				value.name
			}),
			chapter_number,
			date_uploaded: Some(value.start_time_stamp),
			url: Some(format!("{BASE_URL}/viewer/{}", value.chapter_id)),
			..Default::default()
		}
	}
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MangaPlusPage {
	pub manga_page: Option<MangaPage>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MangaPage {
	pub image_url: String,
	// pub width: i32,
	// pub height: i32,
	pub encryption_key: Option<String>,
}

// default values
fn default_true() -> bool {
	true
}

fn default_language() -> Option<Language> {
	Some(Language::English)
}

fn default_wsj() -> Option<Label> {
	Some(Label {
		label: Some(LabelCode::WeeklyShonenJump),
	})
}
fn default_wsj_code() -> Option<LabelCode> {
	Some(LabelCode::WeeklyShonenJump)
}
