use crate::M_URL;
use aidoku::{
	Chapter, FilterItem, FilterValue, Manga, Result,
	alloc::{String, Vec, format, vec},
	helpers::uri::encode_uri_component,
	imports::html::Html,
	prelude::*,
	serde::{Deserialize, Deserializer},
};
use zaimanhua_template::models::{ClassifyTag, ListItem, RecommendCategory, manga};

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct NovelDetailData {
	data: Option<NovelDetail>,
}

impl NovelDetailData {
	// a missing novel answers an object without a name
	pub fn into_detail(self) -> Result<NovelDetail> {
		self.data
			.filter(|detail| detail.name.is_some())
			.ok_or_else(|| error!("小说不存在"))
	}
}

#[derive(Deserialize)]
pub struct NovelDetail {
	name: Option<String>,
	cover: Option<String>,
	authors: Option<String>,
	introduction: Option<String>,
	types: Option<Vec<String>>,
	status: Option<String>,
	last_update_chapter_id: Option<i64>,
	last_update_time: Option<i64>,
	#[serde(default)]
	volume: Vec<NovelVolumeTime>,
}

#[derive(Deserialize)]
struct NovelVolumeTime {
	volume_id: i64,
	addtime: Option<i64>,
}

impl NovelDetail {
	fn volume_time(&self, volume_id: i64) -> Option<i64> {
		self.volume
			.iter()
			.find(|volume| volume.volume_id == volume_id)
			.and_then(|volume| volume.addtime)
	}

	pub fn chapters(&self, id: &str, volumes: Vec<NovelVolume>) -> Vec<Chapter> {
		let multi_volume = volumes.len() > 1;
		let name = &encode_uri_component(self.name.as_deref().unwrap_or_default());
		volumes
			.into_iter()
			.rev()
			.flat_map(|volume| {
				let volume_time = self.volume_time(volume.volume_id);
				let volume_name = volume.volume_name.trim();
				let scanlators = (!volume_name.is_empty()).then(|| vec![volume_name.into()]);
				volume.chapters.into_iter().rev().map(move |item| Chapter {
					key: format!("{}/{}", volume.volume_id, item.chapter_id),
					title: novel_title(&volume.volume_name, &item.chapter_name, multi_volume),
					date_uploaded: if Some(item.chapter_id) == self.last_update_chapter_id {
						self.last_update_time
					} else {
						volume_time
					},
					url: Some(format!(
						"{M_URL}/pages/qing/reader?cptid={}&volid={}&novelid={id}&name={name}",
						item.chapter_id, volume.volume_id
					)),
					scanlators: scanlators.clone(),
					..Default::default()
				})
			})
			.collect()
	}

	pub fn into_manga(self, key: &str) -> Manga {
		Manga {
			authors: self
				.authors
				.map(|authors| authors.split('/').map(Into::into).collect()),
			description: self.introduction,
			url: Some(format!("{M_URL}/pages/qing/detail?id={key}")),
			tags: self.types.map(|types| {
				types
					.iter()
					.flat_map(|tags| tags.split('/'))
					.filter(|tag| !tag.is_empty())
					.map(Into::into)
					.collect()
			}),
			..manga(
				key.into(),
				self.name.unwrap_or_default(),
				self.cover,
				self.status.as_deref(),
			)
		}
	}
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct NovelVolumesData {
	pub data: Option<Vec<NovelVolume>>,
}

#[derive(Deserialize)]
pub struct NovelVolume {
	volume_id: i64,
	#[serde(deserialize_with = "unescape")]
	volume_name: String,
	#[serde(default)]
	chapters: Vec<NovelChapter>,
}

#[derive(Deserialize)]
struct NovelChapter {
	chapter_id: i64,
	#[serde(default, deserialize_with = "unescape")]
	chapter_name: String,
}

fn unescape<'de, D: Deserializer<'de>>(deserializer: D) -> core::result::Result<String, D::Error> {
	let text = String::deserialize(deserializer)?;
	Ok(if text.contains('&') {
		Html::unescape(&text).unwrap_or(text)
	} else {
		text
	})
}

pub fn novel_title(volume_name: &str, name: &str, multi_volume: bool) -> Option<String> {
	let volume_name = volume_name.trim();
	let name = name.trim();
	let title = name
		.strip_prefix(volume_name)
		.filter(|rest| rest.starts_with(char::is_whitespace))
		.map_or(name, str::trim_start);
	let volume_word = volume_name
		.split_whitespace()
		.next()
		.filter(|word| multi_volume && !title.starts_with(word));
	let title = match volume_word {
		Some(word) if title.is_empty() => word.into(),
		Some(word) => format!("{word} {title}"),
		None => title.into(),
	};
	(!title.is_empty()).then_some(title)
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct NovelRecommendData {
	pub recommend_list: Vec<RecommendCategory>,
}

#[derive(Deserialize)]
pub struct HotKeyword {
	keyword: String,
}

impl From<HotKeyword> for FilterItem {
	fn from(item: HotKeyword) -> Self {
		Self {
			title: item.keyword.clone(),
			values: Some(vec![FilterValue::Text {
				id: "keyword".into(),
				value: item.keyword,
			}]),
		}
	}
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct NovelTagData {
	pub tag_list: Vec<ClassifyTag>,
}

// past the last page `rankList` is null
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct RankData {
	pub rank_list: Option<Vec<ListItem>>,
}
