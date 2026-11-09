use aidoku::{
	Chapter, Manga, MangaWithChapter,
	alloc::{String, Vec, collections::BTreeMap, string::ToString, vec},
	prelude::*,
};
use serde::Deserialize;

use crate::helpers::{
	creator_names, format_number, last_segment, manga_url, parse_content_rating, parse_status,
	parse_viewer,
};

// `/manhwa/nuevos`

#[derive(Deserialize)]
pub struct NuevosResponse {
	pub top: TopDto,
	#[serde(rename = "utimos_mangas_creados", default)]
	pub new_series: Vec<LibraryItemDto>,
	#[serde(rename = "utimos_mangas_creados_18", default)]
	pub new_series_18: Vec<LibraryItemDto>,
}

#[derive(Deserialize)]
pub struct TopDto {
	#[serde(rename = "manhwas_esp", default)]
	pub weekly: Vec<PopularItemDto>,
	#[serde(rename = "manhwas_raw", default)]
	pub total: Vec<PopularItemDto>,
}

impl TopDto {
	/// Both rankings merged, without duplicates, most viewed first.
	pub fn into_mangas(self) -> Vec<Manga> {
		let mut items: Vec<PopularItemDto> = Vec::new();
		for item in self.weekly.into_iter().chain(self.total) {
			if !items.iter().any(|seen| seen.slug() == item.slug()) {
				items.push(item);
			}
		}
		items.sort_by_key(|item| core::cmp::Reverse(item.views));
		items.into_iter().map(PopularItemDto::into_manga).collect()
	}
}

#[derive(Deserialize)]
pub struct PopularItemDto {
	/// A path like `/manhwa/{slug}` or `/manga/{slug}`.
	link: String,
	#[serde(rename = "numero", default)]
	views: i64,
	name: String,
	#[serde(rename = "imagen")]
	cover: Option<String>,
}

impl PopularItemDto {
	fn slug(&self) -> &str {
		last_segment(&self.link)
	}

	fn into_manga(self) -> Manga {
		let key = self.slug().to_string();
		Manga {
			url: Some(manga_url(&key)),
			key,
			title: self.name.trim().into(),
			cover: self.cover,
			..Default::default()
		}
	}
}

// `/latest/new-manhwa`

#[derive(Deserialize)]
pub struct LatestResponse {
	pub manhwas: LatestDto,
}

#[derive(Deserialize)]
pub struct LatestDto {
	#[serde(rename = "manhwas_esp", default)]
	esp: Vec<LatestItemDto>,
	#[serde(rename = "manhwas_raw", default)]
	raw: Vec<LatestItemDto>,
	#[serde(rename = "_manhwas", default)]
	esp_18: Vec<LatestItemDto>,
}

impl LatestDto {
	/// All three lists merged, without duplicates, most recently updated first.
	pub fn into_items(self) -> Vec<LatestItemDto> {
		let mut items: Vec<LatestItemDto> = Vec::new();
		for item in self.esp.into_iter().chain(self.raw).chain(self.esp_18) {
			if !items.iter().any(|seen| seen.slug == item.slug) {
				items.push(item);
			}
		}
		items.sort_by_key(|item| core::cmp::Reverse(item.updated_at));
		items
	}
}

#[derive(Deserialize)]
pub struct LatestItemDto {
	#[serde(rename = "id_rel")]
	slug: String,
	#[serde(rename = "name_manhwa")]
	name: String,
	#[serde(rename = "img")]
	cover: Option<String>,
	/// Newest chapter number.
	chapter: Option<f32>,
	#[serde(rename = "gru_name")]
	group: Option<String>,
	/// Milliseconds.
	#[serde(rename = "create", default)]
	updated_at: i64,
}

impl LatestItemDto {
	pub fn into_manga(self) -> Manga {
		Manga {
			url: Some(manga_url(&self.slug)),
			key: self.slug,
			title: self.name.trim().into(),
			cover: self.cover,
			..Default::default()
		}
	}

	/// The series with its newest chapter, for the home page. The API doesn't give
	/// the chapter link, so the key assumes the first version (`_01`), like the site.
	pub fn into_manga_with_chapter(self) -> MangaWithChapter {
		let chapter = Chapter {
			key: format!(
				"{}-{}_01",
				self.slug,
				format_number(self.chapter.unwrap_or(0.0))
			),
			chapter_number: self.chapter,
			date_uploaded: Some(self.updated_at / 1000),
			scanlators: self
				.group
				.as_deref()
				.map(str::trim)
				.filter(|group| !group.is_empty())
				.map(|group| vec![group.into()]),
			..Default::default()
		};
		MangaWithChapter {
			manga: self.into_manga(),
			chapter,
		}
	}
}

// `/manhwa/library`

#[derive(Deserialize)]
pub struct LibraryResponse {
	pub data: Vec<LibraryItemDto>,
	pub next: bool,
}

#[derive(Deserialize)]
pub struct LibraryItemDto {
	real_id: String,
	the_real_name: String,
	#[serde(rename = "_imagen")]
	cover: Option<String>,
	#[serde(rename = "_status")]
	status: Option<String>,
	#[serde(rename = "_erotico")]
	erotic: Option<String>,
}

impl LibraryItemDto {
	pub fn into_manga(self) -> Manga {
		Manga {
			url: Some(manga_url(&self.real_id)),
			key: self.real_id,
			title: self.the_real_name.trim().into(),
			cover: self.cover,
			status: parse_status(self.status.as_deref()),
			content_rating: parse_content_rating(self.erotic.as_deref(), false),
			..Default::default()
		}
	}
}

// `/manhwa/see/{slug}`

#[derive(Deserialize)]
pub struct DetailsDto {
	/// Old id. Chapter links still use it, but the API only accepts `real_id`.
	#[serde(rename = "_id")]
	id: String,
	real_id: String,
	name_esp: Option<String>,
	the_real_name: Option<String>,
	#[serde(rename = "_sinopsis")]
	description: Option<String>,
	#[serde(rename = "_status")]
	status: Option<String>,
	#[serde(rename = "_imagen")]
	cover: Option<String>,
	#[serde(rename = "_tipo")]
	kind: Option<String>,
	#[serde(rename = "_erotico")]
	erotic: Option<String>,
	/// Each genre is a single-entry map like `{"3": "Accion"}`.
	#[serde(rename = "_categoris", default)]
	genres: Vec<BTreeMap<String, String>>,
	#[serde(default)]
	others_name: Vec<String>,
	#[serde(rename = "autor", default)]
	authors: Vec<CreatorDto>,
	#[serde(rename = "artista", default)]
	artists: Vec<CreatorDto>,
	#[serde(default)]
	chapters: Vec<ChapterDto>,
}

#[derive(Deserialize)]
pub struct CreatorDto {
	pub name: String,
}

#[derive(Deserialize)]
struct ChapterDto {
	chapter: f32,
	#[serde(default)]
	versions: Vec<ChapterVersionDto>,
}

#[derive(Deserialize)]
struct ChapterVersionDto {
	/// Like `https://manhwaweb.com/leer/{_id}-{number}_{version}`.
	link: String,
	#[serde(default)]
	joint: Vec<CreatorDto>,
	/// Milliseconds. Missing for chapters that aren't released yet.
	create: Option<i64>,
}

impl DetailsDto {
	pub fn update_manga(&self, manga: &mut Manga) {
		let title = self
			.name_esp
			.as_deref()
			.or(self.the_real_name.as_deref())
			.map(str::trim)
			.filter(|title| !title.is_empty());
		if let Some(title) = title {
			manga.title = title.into();
		}

		let mut description = self
			.description
			.as_deref()
			.map(str::trim)
			.unwrap_or_default()
			.to_string();
		let alt_names = self
			.others_name
			.iter()
			.map(|name| name.trim())
			.filter(|name| !name.is_empty())
			.collect::<Vec<_>>();
		if !alt_names.is_empty() {
			if !description.is_empty() {
				description.push_str("\n\n");
			}
			description.push_str("Nombres alternativos: ");
			description.push_str(&alt_names.join(", "));
		}

		let tags: Vec<String> = self
			.genres
			.iter()
			.filter_map(|genre| genre.values().next())
			.map(|name| name.trim().into())
			.collect();
		let is_ecchi = tags.iter().any(|tag| tag == "Ecchi");

		manga.key = self.real_id.clone();
		manga.url = Some(manga_url(&self.real_id));
		manga.cover = self.cover.clone().or(manga.cover.take());
		manga.description = Some(description).filter(|d| !d.is_empty());
		manga.authors = creator_names(&self.authors);
		manga.artists = creator_names(&self.artists);
		manga.status = parse_status(self.status.as_deref());
		manga.viewer = parse_viewer(self.kind.as_deref());
		manga.content_rating = parse_content_rating(self.erotic.as_deref(), is_ecchi);
		manga.tags = Some(tags).filter(|tags| !tags.is_empty());
	}

	/// Every version of every released chapter, newest first.
	pub fn chapters(&self) -> Vec<Chapter> {
		let mut chapters: Vec<Chapter> = self
			.chapters
			.iter()
			.flat_map(|chapter| {
				chapter
					.versions
					.iter()
					.filter_map(move |version| self.parse_version(chapter.chapter, version))
			})
			.collect();
		chapters.sort_by(|a, b| {
			b.chapter_number
				.partial_cmp(&a.chapter_number)
				.unwrap_or(core::cmp::Ordering::Equal)
		});
		chapters
	}

	fn parse_version(&self, number: f32, version: &ChapterVersionDto) -> Option<Chapter> {
		let date = version.create?;
		// Links use the old id, which the API doesn't accept for some series.
		let url = version.link.replace(&self.id, &self.real_id);
		let mut scanlators: Vec<String> = version
			.joint
			.iter()
			.map(|group| group.name.trim())
			.filter(|name| !name.is_empty())
			.map(String::from)
			.collect();
		if version.joint.is_empty() {
			scanlators.push("ManhwaWeb Bot".into());
		}
		Some(Chapter {
			key: last_segment(&url).into(),
			chapter_number: Some(number),
			date_uploaded: Some(date / 1000),
			scanlators: Some(scanlators).filter(|s| !s.is_empty()),
			url: Some(url),
			..Default::default()
		})
	}
}

// `/chapters/see/{key}`

#[derive(Deserialize)]
pub struct PagesResponse {
	pub chapter: PagesDto,
}

#[derive(Deserialize)]
pub struct PagesDto {
	#[serde(default)]
	pub img: Vec<String>,
}
