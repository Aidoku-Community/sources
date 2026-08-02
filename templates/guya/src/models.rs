use aidoku::{
	Manga,
	alloc::{format, string::String, vec::Vec},
};
use core::fmt;
use serde::{
	Deserialize, Deserializer,
	de::{MapAccess, Visitor},
};

fn null_as_empty<'de, D: Deserializer<'de>>(d: D) -> core::result::Result<String, D::Error> {
	let opt: Option<String> = Option::deserialize(d)?;
	Ok(opt.unwrap_or_default())
}

// GET /api/get_all_series/ → { "Series Title": AllSeriesItem, ... }
#[derive(Deserialize, Clone)]
pub struct AllSeriesItem {
	#[serde(default)]
	pub slug: String,
	#[serde(default, deserialize_with = "null_as_empty")]
	pub cover: String,
	#[serde(default)]
	pub last_updated: i64,
}

impl AllSeriesItem {
	pub fn into_manga(self, title: String, base_url: &str) -> Manga {
		let url = format!("{base_url}/read/manga/{}/", self.slug);
		Manga {
			key: self.slug,
			title,
			cover: if self.cover.is_empty() {
				None
			} else {
				Some(format!("{base_url}{}", self.cover))
			},
			url: Some(url),
			..Default::default()
		}
	}
}

// GET /api/series/{slug}/
#[derive(Deserialize)]
pub struct SeriesDetail {
	pub slug: String,
	pub title: String,
	#[serde(default)]
	pub description: String,
	#[serde(default)]
	pub author: String,
	#[serde(default)]
	pub artist: String,
	#[serde(default)]
	pub cover: String,
	#[serde(default)]
	pub adult: bool,
	#[serde(default)]
	pub groups: GroupsMap,
	#[serde(default)]
	pub chapters: ChaptersMap,
}

// group_id → group_name
#[derive(Default)]
pub struct GroupsMap(pub Vec<(String, String)>);

impl<'de> Deserialize<'de> for GroupsMap {
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		struct V;
		impl<'de> Visitor<'de> for V {
			type Value = GroupsMap;
			fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
				f.write_str("groups map")
			}
			fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
				let mut items = Vec::new();
				while let Some((k, v)) = m.next_entry::<String, String>()? {
					items.push((k, v));
				}
				Ok(GroupsMap(items))
			}
		}
		d.deserialize_map(V)
	}
}

impl GroupsMap {
	pub fn get(&self, id: &str) -> Option<&str> {
		self.0
			.iter()
			.find(|(k, _)| k == id)
			.map(|(_, v)| v.as_str())
	}
}

// chapter_num_str → ChapterData
#[derive(Default)]
pub struct ChaptersMap(pub Vec<(String, ChapterData)>);

impl<'de> Deserialize<'de> for ChaptersMap {
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		struct V;
		impl<'de> Visitor<'de> for V {
			type Value = ChaptersMap;
			fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
				f.write_str("chapters map")
			}
			fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
				let mut items = Vec::new();
				while let Some((k, v)) = m.next_entry::<String, ChapterData>()? {
					items.push((k, v));
				}
				Ok(ChaptersMap(items))
			}
		}
		d.deserialize_map(V)
	}
}

impl ChaptersMap {
	pub fn find(&self, key: &str) -> Option<&ChapterData> {
		self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
	}

	pub fn find_by_folder(&self, folder: &str) -> Option<&ChapterData> {
		self.0
			.iter()
			.find(|(_, v)| v.folder == folder)
			.map(|(_, v)| v)
	}
}

fn default_true() -> bool {
	true
}

#[derive(Deserialize)]
pub struct ChapterData {
	pub folder: String,
	#[serde(default = "default_true")]
	pub is_public: bool,
	pub title: Option<String>,
	pub volume: Option<String>,
	#[serde(default)]
	pub groups: ChapterGroupsMap,
	#[serde(default)]
	pub release_date: ReleaseDate,
}

// group_id → Vec<filename>
#[derive(Default)]
pub struct ChapterGroupsMap(pub Vec<(String, Vec<String>)>);

impl<'de> Deserialize<'de> for ChapterGroupsMap {
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		struct V;
		impl<'de> Visitor<'de> for V {
			type Value = ChapterGroupsMap;
			fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
				f.write_str("chapter groups map")
			}
			fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
				let mut items = Vec::new();
				while let Some((k, v)) = m.next_entry::<String, Vec<String>>()? {
					items.push((k, v));
				}
				Ok(ChapterGroupsMap(items))
			}
		}
		d.deserialize_map(V)
	}
}

impl ChapterGroupsMap {
	pub fn group_ids(&self) -> impl Iterator<Item = &str> {
		self.0.iter().map(|(k, _)| k.as_str())
	}

	pub fn get(&self, id: &str) -> Option<&[String]> {
		self.0
			.iter()
			.find(|(k, _)| k == id)
			.map(|(_, v)| v.as_slice())
	}
}

// release_date: group_id → Unix timestamp
#[derive(Default)]
pub struct ReleaseDate(pub Vec<(String, i64)>);

impl ReleaseDate {
	pub fn get(&self, id: &str) -> Option<i64> {
		self.0.iter().find(|(k, _)| k == id).map(|(_, v)| *v)
	}
}

impl<'de> Deserialize<'de> for ReleaseDate {
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		struct V;
		impl<'de> Visitor<'de> for V {
			type Value = ReleaseDate;
			fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
				f.write_str("release date map")
			}
			fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
				let mut items = Vec::new();
				while let Some((k, ts)) = m.next_entry::<String, i64>()? {
					items.push((k, ts));
				}
				Ok(ReleaseDate(items))
			}
		}
		d.deserialize_map(V)
	}
}
