use crate::{
	BASE_URL,
	settings::{self, CONTENT_RATINGS},
};
use aidoku::{
	Chapter, Result,
	alloc::{String, Vec, collections::BTreeMap, format, string::ToString, vec},
	helpers::uri::encode_uri_component,
};
use core::cmp::Ordering;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use spin::Lazy;

// The site's "Vol.N Ch.N" or "Chapter N" prefix; a bare number can begin a title ("12 Days Later").
static NUMBER_PREFIX: Lazy<Regex> = Lazy::new(|| {
	Regex::new(
		r"(?i)^(?:vol(?:ume)?\.?\s*\d+(?:\.\d+)?\s*)?ch(?:apter)?\.?\s*(\d+(?:\.\d+)?)(?:\s*[:.-]\s*|\s+|$)",
	)
	.unwrap()
});

// The site's own settings, which its pages apply on the server.
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSettings {
	pub hidden_types: Vec<String>,
	pub hidden_ratings: Vec<String>,
	pub hidden_tags: Vec<String>,
	pub chapter_langs: Vec<String>,
	pub title_lang: String,
	pub official_only: bool,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub home: Vec<String>,
}

impl SiteSettings {
	pub fn current() -> Self {
		let ratings = settings::content_ratings();
		Self {
			hidden_types: settings::excluded_types(),
			hidden_ratings: CONTENT_RATINGS
				.iter()
				.filter(|rating| !ratings.iter().any(|shown| shown == *rating))
				.map(|rating| (*rating).into())
				.collect(),
			hidden_tags: settings::excluded_tags()
				.into_iter()
				.map(|tag| tag.to_lowercase())
				.collect(),
			chapter_langs: settings::languages()
				.into_iter()
				.map(|language| language.to_lowercase())
				.collect(),
			title_lang: settings::title_preference(),
			official_only: settings::official_only(),
			home: Vec::new(),
		}
	}

	// The site reads every rating hidden as none hidden, so a cleared setting is checked here.
	pub fn shows_nothing(&self) -> bool {
		self.chapter_langs.is_empty() || self.hidden_ratings.len() == CONTENT_RATINGS.len()
	}

	pub fn cookie(&self) -> Result<String> {
		let json = serde_json::to_string(self)?;
		Ok(format!(
			"yf.adult=1; yf.settings={}",
			encode_uri_component(json)
		))
	}
}

#[derive(Deserialize)]
pub struct FilterLists {
	pub genres: Vec<Named>,
	pub tags: Vec<Named>,
	pub years: Vec<i32>,
}

#[derive(Deserialize)]
pub struct Named {
	pub name: String,
}

#[derive(Deserialize)]
pub struct ChapterData {
	pub groups: Vec<Group>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
	label: String,
	num: Option<f32>,
	#[serde(default)]
	num_real: u8,
	vol: Option<f32>,
	created_at: i64,
	#[serde(default)]
	extra: bool,
	variants: Vec<Variant>,
}

#[derive(Deserialize)]
pub struct Variant {
	id: u64,
	s: u64,
	lang: Option<String>,
	#[serde(default)]
	d: Value,
}

#[derive(Deserialize)]
pub struct ReaderResponse {
	pub chapter: ReaderChapter,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderChapter {
	pub title_id: Option<u64>,
	pub pages: Vec<ReaderPage>,
	pub sealed: Option<String>,
	#[serde(default)]
	pub sources: BTreeMap<String, String>,
}

#[derive(Deserialize)]
pub struct ReaderPage {
	pub hash: String,
	pub w: Option<u32>,
}

#[derive(Deserialize)]
pub struct ErrorResponse {
	pub error: String,
}

impl ChapterData {
	pub fn first_key(&self) -> Option<String> {
		Some(self.groups.first()?.variants.first()?.key())
	}
}

impl Group {
	fn numbered(&self) -> Option<f32> {
		self.num.filter(|num| self.num_real == 1 && num.is_finite())
	}

	// An extra's number is a running count ("Bounus.7" is 164), unless its label names the same one.
	fn chapter_number(&self) -> Option<f32> {
		self.numbered()
			.filter(|num| !self.extra || split_label(&self.label).0 == Some(*num))
	}

	fn title(&self) -> Option<String> {
		match split_label(&self.label) {
			(Some(number), title) if self.chapter_number() == Some(number) => title,
			_ => Some(self.label.clone()),
		}
	}

	// Some uploads have no date, which the site sends as 0.
	fn date(&self) -> Option<i64> {
		(self.created_at > 0).then_some(self.created_at / 1000)
	}
}

// A label such as "Chapter 1: Oneshot" splits into its number and the title after it.
pub fn split_label(label: &str) -> (Option<f32>, Option<String>) {
	let Some(captures) = NUMBER_PREFIX.captures(label) else {
		return (None, Some(label.into()));
	};
	let title = Some(String::from(&label[captures[0].len()..])).filter(|title| !title.is_empty());
	(captures[1].parse().ok(), title)
}

impl Variant {
	// A variant without a language is English, as the site shows it.
	fn language<'a>(&self, languages: &'a [String]) -> Option<&'a String> {
		let lang = self.lang.as_deref().unwrap_or("en");
		languages
			.iter()
			.find(|language| language.eq_ignore_ascii_case(lang))
	}

	fn is_dead(&self) -> bool {
		match &self.d {
			Value::Null => false,
			Value::Bool(dead) => *dead,
			Value::Number(number) => number.as_f64() != Some(0.0),
			_ => true,
		}
	}

	// The reader needs the source series as well as the chapter.
	fn key(&self) -> String {
		format!("{}/{}", self.s, self.id)
	}

	fn chapter(&self, language: &str) -> Chapter {
		let key = self.key();
		Chapter {
			url: Some(format!("{BASE_URL}/chapter/{key}")),
			key,
			language: Some(language.into()),
			..Default::default()
		}
	}
}

pub fn chapter_list(
	data: ChapterData,
	languages: &[String],
	names: &BTreeMap<String, String>,
	clean: bool,
) -> Vec<Chapter> {
	let groups: Vec<_> = data
		.groups
		.into_iter()
		.filter_map(|mut group| {
			group
				.variants
				.retain(|variant| variant.language(languages).is_some());
			(!group.variants.is_empty()).then_some(group)
		})
		.collect();
	if clean {
		let chapters = clean_list(&groups, languages);
		if !chapters.is_empty() {
			return chapters;
		}
	}
	groups
		.iter()
		.flat_map(|group| {
			group.variants.iter().filter_map(move |variant| {
				let language = variant.language(languages)?;
				Some(Chapter {
					title: group.title(),
					chapter_number: group.chapter_number(),
					volume_number: group.vol,
					date_uploaded: group.date(),
					scanlators: names
						.get(&variant.s.to_string())
						.map(|name| vec![name.clone()]),
					..variant.chapter(language)
				})
			})
		})
		.collect()
}

// The site's "Deduplicate chapters & clean order": one row per volume and whole number,
// opening the version of its lowest-numbered part.
fn clean_list(groups: &[Group], languages: &[String]) -> Vec<Chapter> {
	let mut buckets: Vec<((u32, i64), Vec<&Group>)> = Vec::new();
	for group in groups {
		let Some(num) = group.numbered() else {
			continue;
		};
		if group.extra || group.variants.iter().all(Variant::is_dead) {
			continue;
		}
		// Chapter numbers are never negative, so truncating floors them.
		let key = (group.vol.unwrap_or_default().to_bits(), num as i64);
		match buckets.iter_mut().find(|(bucket, _)| *bucket == key) {
			Some((_, members)) => members.push(group),
			None => buckets.push((key, vec![group])),
		}
	}
	buckets
		.iter()
		.filter_map(|((_, number), members)| {
			let first = members
				.iter()
				.min_by(|a, b| a.num.partial_cmp(&b.num).unwrap_or(Ordering::Equal))?;
			let variant = first.variants.iter().find(|variant| !variant.is_dead())?;
			let volumes = buckets.iter().filter(|((_, n), _)| n == number).count();
			Some(Chapter {
				chapter_number: Some(*number as f32),
				volume_number: first.vol.filter(|_| volumes > 1),
				date_uploaded: members.iter().filter_map(|group| group.date()).max(),
				..variant.chapter(variant.language(languages)?)
			})
		})
		.collect()
}
