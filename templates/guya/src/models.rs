use aidoku::{
    Manga,
    alloc::{format, string::String, vec::Vec},
};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use core::fmt;

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
            title: String::from(title.trim()),
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
        self.0.iter().find(|(k, _)| k == id).map(|(_, v)| v.as_str())
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
}

#[derive(Deserialize)]
pub struct ChapterData {
    pub folder: String,
    pub is_public: bool,
    pub title: Option<String>,
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
}

// release_date: group_id → Unix timestamp; only the first value is used.
#[derive(Default)]
pub struct ReleaseDate(pub Option<i64>);

impl<'de> Deserialize<'de> for ReleaseDate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = ReleaseDate;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("release date map")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
                let ts = if let Some((_, ts)) = m.next_entry::<String, i64>()? {
                    Some(ts)
                } else {
                    None
                };
                while m.next_entry::<de::IgnoredAny, de::IgnoredAny>()?.is_some() {}
                Ok(ReleaseDate(ts))
            }
        }
        d.deserialize_map(V)
    }
}
