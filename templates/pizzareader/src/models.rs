use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PizzaResultsDto {
	#[serde(default)]
	pub comics: Vec<PizzaComicDto>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PizzaResultDto {
	#[serde(default)]
	pub comic: Option<PizzaComicDto>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PizzaReaderDto {
	#[serde(default)]
	pub chapter: Option<PizzaChapterDto>,
	#[expect(dead_code)]
	#[serde(default)]
	pub comic: Option<PizzaComicDto>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PizzaComicDto {
	#[serde(default)]
	pub slug: String,
	#[serde(default)]
	pub artist: Option<String>,
	#[serde(default)]
	pub author: String,
	#[serde(default)]
	pub chapters: Vec<PizzaChapterDto>,
	#[serde(default)]
	pub description: String,
	#[serde(default)]
	pub genres: Vec<PizzaGenreDto>,
	#[serde(default)]
	pub last_chapter: Option<PizzaChapterDto>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub title: String,
	#[serde(default)]
	pub thumbnail: String,
	#[serde(default)]
	pub url: String,
	#[serde(default)]
	pub adult: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PizzaGenreDto {
	#[serde(default)]
	pub name: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PizzaChapterDto {
	#[serde(default)]
	pub volume: Option<i32>,
	#[serde(default)]
	pub chapter: Option<i32>,
	#[serde(default)]
	pub full_title: String,
	#[serde(default)]
	pub title: Option<String>,
	#[serde(default)]
	pub pages: Vec<String>,
	#[serde(default)]
	pub published_on: String,
	#[serde(default)]
	pub subchapter: Option<i32>,
	#[serde(default)]
	pub url: String,
}
