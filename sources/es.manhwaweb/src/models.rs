use aidoku::{
	Chapter, Manga, MangaStatus, Viewer,
	alloc::{String, Vec},
	prelude::*,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct NuevosResponse {
	// 'utimos_mangas_creados' (latest_mangas) and 'top' are never used by logic,
	// so we can omit them. If the JSON structure requires them to be present but ignored,
	// we can keep them or use `serde::IgnoredAny` if we want to be strict,
	// but typically just omitting them from the struct works if `deny_unknown_fields` isn't on.

	// However, the `ManhwaWeb` logic accesses `data.manhwas.spanish_manhwas`.
	pub manhwas: ManhwasCollection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManhwasCollection {
	#[serde(rename = "manhwas_esp")]
	pub spanish_manhwas: Vec<UpdateManga>,
}

// TopCollection and TopManga were seemingly unused. Removed.

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateManga {
	pub name_manhwa: String,
	pub img: Option<String>,
	pub id_manhwa: String,
	pub chapter: f32,
	// 'create' was unused.
	pub gru_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryResponse {
	pub data: Vec<LibraryManga>,
	pub next: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryManga {
	#[serde(rename = "_id")]
	pub id: String,
	pub the_real_name: String,
	#[serde(rename = "_imagen")]
	pub image: Option<String>,
	#[serde(rename = "_status")]
	pub status: Option<String>,
	#[serde(rename = "_erotico")]
	pub erotic: Option<String>,
	#[serde(rename = "_categoris")]
	pub categories: Option<Vec<i32>>,
}

impl LibraryManga {
	pub fn to_manga(self, base_url: &str) -> Manga {
		let url = Some(format!("{}/manhwa/{}", base_url, self.id));
		Manga {
			key: self.id.into(),
			title: self.the_real_name,
			cover: self.image.map(|s| s.into()),
			url,
			status: self
				.status
				.as_ref()
				.map(|s| match s.as_str() {
					"publicandose" => MangaStatus::Ongoing,
					"finalizado" => MangaStatus::Completed,
					"pausado" => MangaStatus::Hiatus,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or(MangaStatus::Unknown),
			..Default::default()
		}
	}
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeeResponse {
	#[serde(rename = "_id")]
	pub id: String,
	pub the_real_name: String,
	#[serde(rename = "_sinopsis")]
	pub synopsis: Option<String>,
	#[serde(rename = "_imagen")]
	pub image: Option<String>,
	#[serde(rename = "_status")]
	pub status: Option<String>,
	#[serde(rename = "_tipo")]
	pub serie_type: Option<String>,
	// Removed unused 'demography' and 'erotic'
	#[serde(rename = "_categoris")]
	pub categories: Option<Vec<serde_json::Value>>,
	pub chapters: Vec<RawSeeChapter>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawSeeChapter {
	pub chapter: f32,
	pub create: i64,
	pub link: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChapterSeeResponse {
	pub chapter: ChapterImgData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChapterImgData {
	pub img: Vec<String>,
}

impl SeeResponse {
	pub fn parse_manga(&self, base_url: &str) -> Manga {
		Manga {
			key: self.id.clone().into(),
			title: self.the_real_name.clone(),
			description: self.synopsis.as_ref().map(|s| s.trim().into()),
			cover: self.image.as_ref().map(|s| s.into()),
			url: Some(format!("{base_url}/manhwa/{}", self.id)),
			status: self
				.status
				.as_ref()
				.map(|s| match s.as_str() {
					"publicandose" => MangaStatus::Ongoing,
					"finalizado" => MangaStatus::Completed,
					"pausado" => MangaStatus::Hiatus,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or(MangaStatus::Unknown),
			viewer: self
				.serie_type
				.as_ref()
				.map(|s| match s.as_str() {
					"manhwa" | "manhua" => Viewer::Webtoon,
					_ => Viewer::RightToLeft,
				})
				.unwrap_or(Viewer::Unknown),
			tags: self
				.categories
				.as_ref()
				.map(|cats: &Vec<serde_json::Value>| {
					cats.iter()
						.filter_map(|cat: &serde_json::Value| {
							cat.as_object()
								.and_then(|obj: &serde_json::Map<String, serde_json::Value>| {
									obj.values().next()
								})
								.and_then(|v: &serde_json::Value| v.as_str())
								.map(|s: &str| s.into())
						})
						.collect()
				}),
			..Default::default()
		}
	}

	pub fn parse_chapters(&self, _base_url: &str) -> Vec<Chapter> {
		let mut chapters: Vec<Chapter> = self
			.chapters
			.iter()
			.map(|c| {
				let url = Some(c.link.clone());
				Chapter {
					// The URL is like /leer/slug-number
					// But the API for images uses the whole slug-number as ID (KEY)
					key: c.link.rsplit('/').next().unwrap_or(&self.id).into(),
					chapter_number: Some(c.chapter),
					date_uploaded: Some(c.create / 1000), // convert ms to s
					url,
					..Default::default()
				}
			})
			.collect();

		// Sort by chapter number descending
		chapters.sort_by(|a, b| {
			b.chapter_number
				.partial_cmp(&a.chapter_number)
				.unwrap_or(core::cmp::Ordering::Equal)
		});

		chapters
	}
}
