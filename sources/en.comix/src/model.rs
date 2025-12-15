// use crate::Params;
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, fmt, string::ToString, vec},
	helpers::element::ElementHelpers,
	imports::html::Html,
	prelude::*,
};
use chrono::DateTime;
use serde::de::{self, Deserializer, Visitor};
use serde::{self, Deserialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComixResponse<T> {
	pub status: i64,
	pub result: ResultData<T>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultData<T> {
	pub items: Vec<T>,
	pub pagination: Pagination,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComixManga<'a> {
	pub manga_id: i64,
	pub hash_id: &'a str,
	pub title: String,
	pub alt_titles: Vec<String>,
	pub synopsis: String,
	pub slug: &'a str,
	pub rank: i64,
	#[serde(rename = "type")]
	pub type_: ComixTypeFilter,

	pub poster: Poster<'a>,

	pub original_language: Option<String>,
	pub status: ComixStatus,

	pub final_volume: i64,
	pub final_chapter: i64,

	pub has_chapters: bool,
	pub latest_chapter: f64,

	pub chapter_updated_at: i64,

	pub start_date: i64,
	pub end_date: String,

	pub created_at: i64,
	pub updated_at: i64,

	pub rated_avg: f64,
	pub rated_count: i64,
	pub follows_total: i64,

	pub links: Links,

	pub is_nsfw: bool,

	pub year: i64,
	pub term_ids: Vec<i64>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ComixChapter {
	pub chapter_id: i64,
	pub manga_id: i64,

	pub scanlation_group_id: i64,

	/// In your JSON it's `0`/`1` (number), not `true`/`false`.
	pub is_official: i64,

	pub number: f64,
	pub name: String,
	pub language: String,

	pub volume: i64,
	pub votes: i64,

	pub created_at: i64,
	pub updated_at: i64,

	pub scanlation_group: Option<ScanlationGroup>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ChapterResponse {
	pub status: i64,
	pub result: Option<Item>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct Item {
	pub chapter_id: i64,
	pub images: Vec<Images>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct Images {
	pub url: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ScanlationGroup {
	pub scanlation_group_id: i64,
	pub name: String,
	pub slug: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Poster<'a> {
	pub small: &'a str,
	pub medium: &'a str,
	pub large: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Links {
	pub al: Option<String>,
	pub mal: Option<String>,
	pub mu: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
	pub count: i64,
	pub total: i64,
	pub per_page: i64,
	pub current_page: i64,
	pub last_page: i64,
	pub from: i64,
	pub to: i64,
}

impl ComixManga<'_> {
	pub fn into_basic_manga(self) -> Manga {
		Manga {
			key: String::from(self.hash_id),
			title: self.title().unwrap_or_default(),
			cover: self.cover(),
			..Default::default()
		}
	}

	pub fn title(&self) -> Option<String> {
		self.alt_titles
			.first()
			.map(|t| t.to_string())
			.or_else(|| Some(self.title.clone()))
	}

	pub fn description(&self) -> Option<String> {
		Some(self.synopsis)
	}

	pub fn cover(&self) -> Option<String> {
		// self.relationships.iter().find_map(|r| {
		// 	if r.r#type == "cover_art" {
		// 		Some(format!(
		// 			"{COVER_URL}/covers/{}/{}{}",
		// 			self.id,
		// 			r.attributes
		// 				.clone()
		// 				.and_then(|v| v.get("fileName").map(|v| v.as_str().map(String::from)))
		// 				.flatten()
		// 				.unwrap_or_default(),
		// 			settings::get_cover_quality()
		// 		))
		// 	} else {
		// 		None
		// 	}
		// })
		todo!()
	}

	pub fn authors(&self) -> Vec<String> {
		// self.relationships
		// 	.iter()
		// 	.filter(|r| r.r#type == "author")
		// 	.filter_map(|r| {
		// 		r.attributes
		// 			.as_ref()
		// 			.map(|a| a.get("name").map(|v| v.as_str().map(String::from)))
		// 	})
		// 	.flatten()
		// 	.flatten()
		// 	.collect()
		todo!()
	}

	pub fn artists(&self) -> Vec<String> {
		// self.relationships
		// 	.iter()
		// 	.filter(|r| r.r#type == "artist")
		// 	.filter_map(|r| {
		// 		r.attributes
		// 			.as_ref()
		// 			.map(|a| a.get("name").map(|v| v.as_str().map(String::from)))
		// 	})
		// 	.flatten()
		// 	.flatten()
		// 	.collect()
		todo!()
	}

	pub fn url(&self) -> String {
		format!("https://comix.com/title/{}", self.hash_id)
	}

	pub fn tags(&self) -> Vec<String> {
		self.term_ids
			.iter()
			// .filter_map(|t| t.attributes.name.get())
			.map(|t| t.to_string())
			.collect()
	}

	pub fn status(&self) -> MangaStatus {
		match self.status {
			ComixStatus::Releasing => MangaStatus::Ongoing,
			ComixStatus::Finished => MangaStatus::Completed,
			ComixStatus::OnHiatus => MangaStatus::Hiatus,
			ComixStatus::Discontinued => MangaStatus::Cancelled,
			ComixStatus::NotYetReleased => MangaStatus::Unknown,
		}
	}

	pub fn content_rating(&self) -> ContentRating {
		if self.is_nsfw {
			ContentRating::NSFW
		} else {
			ContentRating::Safe
		}
	}
}

impl From<ComixManga<'_>> for Manga {
	fn from(val: ComixManga<'_>) -> Self {
		let tags = val.tags();
		let viewer = match val.type_ {
			ComixTypeFilter::Manga => Viewer::RightToLeft,
			ComixTypeFilter::Manhwa => Viewer::Webtoon,
			ComixTypeFilter::Manhua => Viewer::Webtoon,
			_ => Viewer::Webtoon,
		};

		Manga {
			key: String::from(val.hash_id),
			title: val.title().unwrap_or_default(),
			cover: val.cover(),
			artists: Some(val.artists()),
			authors: Some(val.authors()),
			description: val.description(),
			url: Some(val.url()),
			tags: Some(tags),
			status: val.status(),
			content_rating: val.content_rating(),
			viewer,
			..Default::default()
		}
	}
}

impl ComixChapter {
	pub fn has_external_url(&self) -> bool {
		false
	}

	pub fn url(&self, manga: &Manga) -> String {
		match manga.url.as_deref() {
			Some(base) => format!("{}/{}", base, self.chapter_id),
			None => String::new(),
		}
	}

	pub fn manga_id(&self) -> Option<i64> {
		self.chapter_id.into()
	}

	pub fn scanlators(&self) -> Vec<String> {
		match self.scanlation_group.as_ref() {
			Some(group) => vec![group.name.clone()],
			None => Vec::new(),
		}
	}
}

impl From<ComixChapter> for Chapter {
	fn from(val: ComixChapter) -> Self {
		let chapter_number = Some(val.number as f32);
		let volume_number = Some(val.volume as f32);

		// As per MangaDex upload guidelines, if the volume and chapter are both null or
		// for serialized entries, the volume is 0 and chapter is null, it's a oneshot.
		// They should have a title of "Oneshot" but some don't, so we'll add it if it's missing.
		let title = if (volume_number == Some(0.0)) && val.name.is_empty() {
			Some(String::from("Oneshot"))
		} else {
			Some(val.name.clone())
		};

		Chapter {
			key: String::from(val.chapter_id.to_string()),
			title,
			chapter_number,
			volume_number,
			date_uploaded: Some(val.updated_at),
			scanlators: Some(val.scanlators()),
			// url: Some(val.url),
			language: Some(String::from(val.language)),
			..Default::default()
		}
	}
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ComixStatus {
	Releasing,
	#[default]
	Finished,
	OnHiatus,
	Discontinued,
	NotYetReleased,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ComixTypeFilter {
	#[default]
	Manga,
	Manhwa,
	Manhua,
	Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
	Action,
	Adult,
	Adventure,
	BoysLove,
	Comedy,
	Crime,
	Drama,
	Ecchi,
	Fantasy,
	GirlsLove,
	Hentai,
	Historical,
	Horror,
	Isekai,
	MagicalGirls,
	Mature,
	Mecha,
	Medical,
	Mystery,
	Philosophical,
	Psychological,
	Romance,
	SciFi,
	SliceOfLife,
	Smut,
	Sports,
	Superhero,
	Thriller,
	Tragedy,
	Wuxia,
	Aliens,
	Animals,
	Cooking,
	CrossDressing,
	Delinquents,
	Demons,
	Genderswap,
	Ghosts,
	Gyaru,
	Harem,
	Incest,
	Loli,
	Mafia,
	Magic,
	MartialArts,
	Military,
	MonsterGirls,
	Monsters,
	Music,
	Ninja,
	OfficeWorkers,
	Police,
	PostApocalyptic,
	Reincarnation,
	ReverseHarem,
	Samurai,
	SchoolLife,
	Shota,
	Supernatural,
	Survival,
	TimeTravel,
	TraditionalGames,
	Vampires,
	VideoGames,
	Villainess,
	VirtualReality,
	Zombies,
	Shoujo,
	Shounen,
	Josei,
	Seinen,
	Fake,
}

impl fmt::Display for Tag {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.name())
	}
}

impl Tag {
	/// The numeric term_id used by the API.
	pub const fn id(self) -> u32 {
		match self {
			Tag::Action => 6,
			Tag::Adult => 87264,
			Tag::Adventure => 7,
			Tag::BoysLove => 8,
			Tag::Comedy => 9,
			Tag::Crime => 10,
			Tag::Drama => 11,
			Tag::Ecchi => 87265,
			Tag::Fantasy => 12,
			Tag::GirlsLove => 13,
			Tag::Hentai => 87266,
			Tag::Historical => 14,
			Tag::Horror => 15,
			Tag::Isekai => 16,
			Tag::MagicalGirls => 17,
			Tag::Mature => 87267,
			Tag::Mecha => 18,
			Tag::Medical => 19,
			Tag::Mystery => 20,
			Tag::Philosophical => 21,
			Tag::Psychological => 22,
			Tag::Romance => 23,
			Tag::SciFi => 24,
			Tag::SliceOfLife => 25,
			Tag::Smut => 87268,
			Tag::Sports => 26,
			Tag::Superhero => 27,
			Tag::Thriller => 28,
			Tag::Tragedy => 29,
			Tag::Wuxia => 30,
			Tag::Aliens => 31,
			Tag::Animals => 32,
			Tag::Cooking => 33,
			Tag::CrossDressing => 34,
			Tag::Delinquents => 35,
			Tag::Demons => 36,
			Tag::Genderswap => 37,
			Tag::Ghosts => 38,
			Tag::Gyaru => 39,
			Tag::Harem => 40,
			Tag::Incest => 41,
			Tag::Loli => 42,
			Tag::Mafia => 43,
			Tag::Magic => 44,
			Tag::MartialArts => 45,
			Tag::Military => 46,
			Tag::MonsterGirls => 47,
			Tag::Monsters => 48,
			Tag::Music => 49,
			Tag::Ninja => 50,
			Tag::OfficeWorkers => 51,
			Tag::Police => 52,
			Tag::PostApocalyptic => 53,
			Tag::Reincarnation => 54,
			Tag::ReverseHarem => 55,
			Tag::Samurai => 56,
			Tag::SchoolLife => 57,
			Tag::Shota => 58,
			Tag::Supernatural => 59,
			Tag::Survival => 60,
			Tag::TimeTravel => 61,
			Tag::TraditionalGames => 62,
			Tag::Vampires => 63,
			Tag::VideoGames => 64,
			Tag::Villainess => 65,
			Tag::VirtualReality => 66,
			Tag::Zombies => 67,
			Tag::Shoujo => 1,
			Tag::Shounen => 2,
			Tag::Josei => 3,
			Tag::Seinen => 4,
			Tag::Fake => 39725,
		}
	}

	pub const fn from_id(id: i64) -> Option<Self> {
		match id {
			6 => Some(Tag::Action),
			87264 => Some(Tag::Adult),
			7 => Some(Tag::Adventure),
			8 => Some(Tag::BoysLove),
			9 => Some(Tag::Comedy),
			10 => Some(Tag::Crime),
			11 => Some(Tag::Drama),
			87265 => Some(Tag::Ecchi),
			12 => Some(Tag::Fantasy),
			13 => Some(Tag::GirlsLove),
			87266 => Some(Tag::Hentai),
			14 => Some(Tag::Historical),
			15 => Some(Tag::Horror),
			16 => Some(Tag::Isekai),
			17 => Some(Tag::MagicalGirls),
			87267 => Some(Tag::Mature),
			18 => Some(Tag::Mecha),
			19 => Some(Tag::Medical),
			20 => Some(Tag::Mystery),
			21 => Some(Tag::Philosophical),
			22 => Some(Tag::Psychological),
			23 => Some(Tag::Romance),
			24 => Some(Tag::SciFi),
			25 => Some(Tag::SliceOfLife),
			87268 => Some(Tag::Smut),
			26 => Some(Tag::Sports),
			27 => Some(Tag::Superhero),
			28 => Some(Tag::Thriller),
			29 => Some(Tag::Tragedy),
			30 => Some(Tag::Wuxia),
			31 => Some(Tag::Aliens),
			32 => Some(Tag::Animals),
			33 => Some(Tag::Cooking),
			34 => Some(Tag::CrossDressing),
			35 => Some(Tag::Delinquents),
			36 => Some(Tag::Demons),
			37 => Some(Tag::Genderswap),
			38 => Some(Tag::Ghosts),
			39 => Some(Tag::Gyaru),
			40 => Some(Tag::Harem),
			41 => Some(Tag::Incest),
			42 => Some(Tag::Loli),
			43 => Some(Tag::Mafia),
			44 => Some(Tag::Magic),
			45 => Some(Tag::MartialArts),
			46 => Some(Tag::Military),
			47 => Some(Tag::MonsterGirls),
			48 => Some(Tag::Monsters),
			49 => Some(Tag::Music),
			50 => Some(Tag::Ninja),
			51 => Some(Tag::OfficeWorkers),
			52 => Some(Tag::Police),
			53 => Some(Tag::PostApocalyptic),
			54 => Some(Tag::Reincarnation),
			55 => Some(Tag::ReverseHarem),
			56 => Some(Tag::Samurai),
			57 => Some(Tag::SchoolLife),
			58 => Some(Tag::Shota),
			59 => Some(Tag::Supernatural),
			60 => Some(Tag::Survival),
			61 => Some(Tag::TimeTravel),
			62 => Some(Tag::TraditionalGames),
			63 => Some(Tag::Vampires),
			64 => Some(Tag::VideoGames),
			65 => Some(Tag::Villainess),
			66 => Some(Tag::VirtualReality),
			67 => Some(Tag::Zombies),
			1 => Some(Tag::Shoujo),
			2 => Some(Tag::Shounen),
			3 => Some(Tag::Josei),
			4 => Some(Tag::Seinen),
			39725 => Some(Tag::Fake),
			_ => None,
		}
	}

	pub const fn name(self) -> &'static str {
		match self {
			Tag::Action => "Action",
			Tag::Adult => "Adult",
			Tag::Adventure => "Adventure",
			Tag::BoysLove => "Boys Love",
			Tag::Comedy => "Comedy",
			Tag::Crime => "Crime",
			Tag::Drama => "Drama",
			Tag::Ecchi => "Ecchi",
			Tag::Fantasy => "Fantasy",
			Tag::GirlsLove => "Girls Love",
			Tag::Hentai => "Hentai",
			Tag::Historical => "Historical",
			Tag::Horror => "Horror",
			Tag::Isekai => "Isekai",
			Tag::MagicalGirls => "Magical Girls",
			Tag::Mature => "Mature",
			Tag::Mecha => "Mecha",
			Tag::Medical => "Medical",
			Tag::Mystery => "Mystery",
			Tag::Philosophical => "Philosophical",
			Tag::Psychological => "Psychological",
			Tag::Romance => "Romance",
			Tag::SciFi => "Sci-Fi",
			Tag::SliceOfLife => "Slice of Life",
			Tag::Smut => "Smut",
			Tag::Sports => "Sports",
			Tag::Superhero => "Superhero",
			Tag::Thriller => "Thriller",
			Tag::Tragedy => "Tragedy",
			Tag::Wuxia => "Wuxia",
			Tag::Aliens => "Aliens",
			Tag::Animals => "Animals",
			Tag::Cooking => "Cooking",
			Tag::CrossDressing => "Cross Dressing",
			Tag::Delinquents => "Delinquents",
			Tag::Demons => "Demons",
			Tag::Genderswap => "Genderswap",
			Tag::Ghosts => "Ghosts",
			Tag::Gyaru => "Gyaru",
			Tag::Harem => "Harem",
			Tag::Incest => "Incest",
			Tag::Loli => "Loli",
			Tag::Mafia => "Mafia",
			Tag::Magic => "Magic",
			Tag::MartialArts => "Martial Arts",
			Tag::Military => "Military",
			Tag::MonsterGirls => "Monster Girls",
			Tag::Monsters => "Monsters",
			Tag::Music => "Music",
			Tag::Ninja => "Ninja",
			Tag::OfficeWorkers => "Office Workers",
			Tag::Police => "Police",
			Tag::PostApocalyptic => "Post-Apocalyptic",
			Tag::Reincarnation => "Reincarnation",
			Tag::ReverseHarem => "Reverse Harem",
			Tag::Samurai => "Samurai",
			Tag::SchoolLife => "School Life",
			Tag::Shota => "Shota",
			Tag::Supernatural => "Supernatural",
			Tag::Survival => "Survival",
			Tag::TimeTravel => "Time Travel",
			Tag::TraditionalGames => "Traditional Games",
			Tag::Vampires => "Vampires",
			Tag::VideoGames => "Video Games",
			Tag::Villainess => "Villainess",
			Tag::VirtualReality => "Virtual Reality",
			Tag::Zombies => "Zombies",
			Tag::Shoujo => "Shoujo",
			Tag::Shounen => "Shounen",
			Tag::Josei => "Josei",
			Tag::Seinen => "Seinen",
			Tag::Fake => "Fake",
		}
	}
}

impl<'de> Deserialize<'de> for Tag {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		struct TagVisitor;

		impl<'de> Visitor<'de> for TagVisitor {
			type Value = Tag;

			fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
				f.write_str("a numeric tag id (i64)")
			}

			fn visit_i64<E>(self, v: i64) -> Result<Tag, E>
			where
				E: de::Error,
			{
				Tag::from_id(v).ok_or_else(|| E::custom(format!("unknown tag id: {v}")))
			}

			fn visit_u64<E>(self, v: u64) -> Result<Tag, E>
			where
				E: de::Error,
			{
				let v = i64::try_from(v).map_err(|_| E::custom("tag id too large"))?;
				self.visit_i64(v)
			}
		}

		deserializer.deserialize_i64(TagVisitor)
	}
}
